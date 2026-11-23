"""Replays the brain's own requests against llama.cpp's llama-server on this machine, for the
Windows beta's speed and memory measurements (CI job `llama-cpu` in .github/workflows/windows-beta.yml).

The requests were recorded on a Mac from the real brain (its rules, prompts and slot choice),
so this needs neither Swift nor Iris: it starts llama-server with the settings the Windows app
will use, sends the same requests in the same order to the same slots, and reports how long
each took against the time limit the brain would give it, plus the server's memory.

usage: python replay.py <llama-server> <model.gguf> <requests.jsonl> <out.json>
                         [--threads N] [--slots N] [--no-repack] [--label text]
"""
import json, os, secrets, statistics, subprocess, sys, threading, time, urllib.request

import psutil

server, gguf, requests_path, out_path = sys.argv[1:5]
args = sys.argv[5:]
opt = lambda k, d=None: args[args.index(k) + 1] if k in args else d
threads = int(opt("--threads", str(psutil.cpu_count(logical=False) or 2)))
label = opt("--label", os.path.basename(gguf))
port = 18080 + secrets.randbelow(1000)
key = secrets.token_hex(16)

# One slot per fixed prompt the brain keeps (four, like the Mac's prefix cache), each with its own
# cache: --kv-unified would let one slot's work evict another's sliding-window cache, and then no
# tone ever reuses its prompt (measured on b12110). 6,144 tokens a slot fits the longest draft.
slots = int(opt("--slots", "4"))
cmd = [server, "-m", gguf, "--host", "127.0.0.1", "--port", str(port), "--api-key", key,
       "-np", str(slots), "-c", str(slots * 6144), "-t", str(threads), "-ngl", "0", "--no-webui",
       # llama-server keeps up to 8 GB of old prompts in RAM by default: too much for an 8 GB PC.
       "--cache-ram", "512"]
if "--no-repack" in args:
    cmd.append("--no-repack")
log = open(out_path + ".server.log", "w")
started = time.monotonic()
proc = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT)
ps = psutil.Process(proc.pid)


def health():
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2) as r:
            return r.status == 200
    except Exception:
        return False


while not health():
    if proc.poll() is not None:
        sys.exit(f"llama-server exited: see {out_path}.server.log")
    if time.monotonic() - started > 300:
        proc.kill(); sys.exit("llama-server never became healthy")
    time.sleep(0.25)
load_s = time.monotonic() - started

peak = {"rss": 0, "private": 0}
stop = threading.Event()


def sample():
    while not stop.is_set():
        try:
            m = ps.memory_info()
            peak["rss"] = max(peak["rss"], m.rss)
            peak["private"] = max(peak["private"], getattr(m, "private", 0) or getattr(m, "uss", 0) or 0)
        except psutil.Error:
            pass
        time.sleep(0.2)


threading.Thread(target=sample, daemon=True).start()

rows = []
for line in open(requests_path, encoding="utf-8"):
    rec = json.loads(line)
    body = dict(rec["body"])
    if rec["slot"] >= 0:
        body["id_slot"] = rec["slot"]
    is_fix = "key=fix:" in rec["tag"]
    prewarm = rec["tag"].startswith("prewarm")
    # The brain's time limit for one attempt (Rewriter.deadline): loading is never counted.
    limit_ms = None if prewarm else (4000 if is_fix else 6000) + body["max_tokens"] * 45
    req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
    t0 = time.monotonic(); first = None; text = ""; timings = {}; cut = False
    with urllib.request.urlopen(req, timeout=900) as r:
        for raw in r:
            # The brain gives up at the limit; past twice that, stop paying runner minutes.
            if limit_ms and (time.monotonic() - t0) * 1000 > 2 * limit_ms:
                cut = True
                break
            s = raw.decode("utf-8").strip()
            if not s.startswith("data: ") or s == "data: [DONE]":
                continue
            obj = json.loads(s[6:])
            timings = obj.get("timings", timings)
            for c in obj.get("choices", []):
                piece = (c.get("delta") or {}).get("content") or ""
                if piece and first is None:
                    first = time.monotonic() - t0
                text += piece
    total_ms = int((time.monotonic() - t0) * 1000)
    rows.append({"i": rec.get("i", len(rows)), "tag": rec["tag"], "slot": rec["slot"], "prewarm": prewarm,
                 "first_ms": None if first is None else int(first * 1000), "total_ms": total_ms, "limit_ms": limit_ms,
                 "prompt_n": timings.get("prompt_n"), "cache_n": timings.get("cache_n"),
                 "predicted_n": timings.get("predicted_n"), "cut": cut, "text": text})
    print(f"{len(rows):4} {rec['tag'][:40]:40} {total_ms/1000:7.2f}s"
          f" limit {'-' if limit_ms is None else f'{limit_ms/1000:.0f}s'} cache_n={timings.get('cache_n')}", flush=True)

stop.set()
proc.terminate()
try:
    proc.wait(timeout=20)
except subprocess.TimeoutExpired:
    proc.kill()

checks = [r for r in rows if not r["prewarm"]]
over = [r for r in checks if r["total_ms"] > r["limit_ms"]]
summary = {
    "label": label, "threads": threads, "repack": "--no-repack" not in args,
    "cpu": f"{psutil.cpu_count(logical=False)} cores / {psutil.cpu_count()} threads",
    "load_s": round(load_s, 1), "checks": len(checks),
    "median_s": round(statistics.median(r["total_ms"] for r in checks) / 1000, 2),
    "slowest_s": round(max(r["total_ms"] for r in checks) / 1000, 2),
    "worst_share_of_limit": round(max(r["total_ms"] / r["limit_ms"] for r in checks), 2),
    "over_limit": len(over),
    "cold_checks": sum(1 for r in checks if not r["cache_n"]),
    "peak_rss_mb": peak["rss"] // 2**20, "peak_private_mb": peak["private"] // 2**20,
}
json.dump({"summary": summary, "rows": rows}, open(out_path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(json.dumps(summary, indent=1))
if os.environ.get("GITHUB_STEP_SUMMARY"):
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
        f.write(f"### {label}\n\n| | |\n|---|---|\n" + "".join(f"| {k} | {v} |\n" for k, v in summary.items()) + "\n")
