# What the scripts in this folder share, in one place. Source it, don't run it; it works in
# zsh and in bash (build-app.sh). Paths without a leading / are from the checkout's root.

# Iris's own flags, spelled as in Sources/IrisCore/LaunchMode.swift.
IRIS_FLAG_MOCK_ANSWERS=--mock-answers
IRIS_FLAG_SELF_CHECK=--self-check
IRIS_FLAG_PRACTICE_BOX=--practice-box

# Sources/Iris/Updater.swift reads these too (the lock and its age, and the log it writes
# update.sh's output to): change them there as well.
IRIS_UPDATE_LOCK=.build/update.lock
IRIS_UPDATE_LOCK_STALE_MINUTES=15
IRIS_UPDATE_LOG="$HOME/Library/Logs/Iris-update.log"

# Eval work never leaves this Mac (CLAUDE.md). Git pathspecs; .gitignore lists the same.
IRIS_EVAL_PATHS=(evals/ 'scripts/eval*' Sources/IrisEval/ Sources/IrisEngine/Engine+Eval.swift)

# The self-signed identity scripts/make-signing-cert.sh creates.
IRIS_SIGNING_NAME="Iris Local Signing"

# Scripts run from Iris's menu get almost no PATH.
iris_use_standard_path() {
  # ~/.cargo/bin: Rust, for the interface (an update started from Iris has a bare PATH).
  export PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"
}

# Rust, for Iris's interface (gpui/). A Mac that has never built it (one that built the Swift
# app, updating from it) gets rustup here, once, without a question: into ~/.cargo and
# ~/.rustup, with no sudo and no change to shell profiles (iris_use_standard_path finds it).
# rustup then fetches the version gpui/rust-toolchain.toml names on the first build.
iris_ensure_rust() {
  command -v cargo >/dev/null && return 0
  echo "→ Installing Rust (once, for Iris's interface: about 450 MB, a minute or so)…"
  curl --proto '=https' --tlsv1.2 -sSf --max-time 300 https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain none --no-modify-path >/dev/null || {
      echo "Couldn't install Rust. Install it from https://rustup.rs, then update again." >&2
      return 1
    }
  export PATH="$HOME/.cargo/bin:$PATH"
  command -v cargo >/dev/null
}

# Where update.sh installs Iris.
iris_installed_app() {
  if [[ -w /Applications ]]; then echo /Applications/Iris.app; else echo "$HOME/Applications/Iris.app"; fi
}

# The identity builds on this Mac sign with, the first of:
#   1. $SIGN_IDENTITY
#   2. the name in <checkout>/.signing-identity (one line, not committed; per Mac)
#   3. "Iris Local Signing", if make-signing-cert.sh created it
#   4. ad hoc ("-")
iris_signing_identity() {
  local root=$1 name=${SIGN_IDENTITY:-}
  if [ -z "$name" ] && [ -f "$root/.signing-identity" ]; then name="$(head -n 1 "$root/.signing-identity")"; fi
  # No -v: macOS lists a self-signed certificate as "not trusted", which codesign doesn't mind,
  # so -v would hide it and every build would quietly fall back to ad hoc. Skip it if expired.
  if [ -z "$name" ] && security find-identity -p codesigning | grep "\"$IRIS_SIGNING_NAME\"" | grep -qv EXPIRED; then
    name=$IRIS_SIGNING_NAME
  fi
  echo "${name:--}"
}

# Checks out commit $2 in a worktree at $1, kept between runs so its build cache stays warm.
iris_worktree_at() {
  if [[ ! -e "$1/.git" ]]; then
    git worktree prune
    git worktree add --detach "$1" "$2"
  else
    git -C "$1" checkout --quiet --detach --force "$2"
    git -C "$1" clean -fdq   # untracked leftovers; ignored files (the build cache) stay
  fi
}

# Seconds since the last key press, click or pointer move. awk reads to the end so ioreg
# never dies of a closed pipe, which pipefail would count as a failure.
iris_idle_seconds() { ioreg -c IOHIDSystem | awk '/HIDIdleTime/ && !done {print int($NF/1e9); done = 1}'; }

# Waits until nobody has touched the Mac for a minute. Gives up after 15 minutes, saying so
# as $1 (the script's name), and exits 2.
iris_wait_until_idle() {
  local waited=0
  until (( $(iris_idle_seconds) >= 60 )); do
    if (( waited >= 900 )); then
      echo "$1: someone is using this Mac (no minute without input in 15 minutes). Try later, or pass --now if the owner said to go ahead." >&2
      exit 2
    fi
    (( waited == 0 )) && echo "Waiting until nobody has touched the Mac for a minute…"
    sleep 10
    (( waited += 10 ))
  done
}

# Waits up to 5 seconds for every Iris (installed or a build) to exit.
iris_wait_until_quit() { for _ in {1..50}; do pgrep -x Iris >/dev/null || break; sleep 0.1; done; }

iris_quit() { pkill -x Iris || true; iris_wait_until_quit; }

# Build settings a script may have in its environment. `open` hands the script's environment
# to the app, and Iris hands its own to update.sh, so none of these may reach Iris: one left
# over made every "Update & reopen" rebuild the version already installed. Updater.swift
# strips the same ones (IrisCore's UpdateEnvironment).
IRIS_BUILD_VARS=(IRIS_COMMIT IRIS_SOURCE_PATH SIGN_IDENTITY)

# open "$@" without the build settings above, retried: Launch Services can refuse right after a
# copy quits (error -600). Only the last try's error is shown.
iris_open() {
  local try v unset_args=()
  for v in "${IRIS_BUILD_VARS[@]}"; do unset_args+=(-u "$v"); done
  for try in 1 2 3 4; do
    env "${unset_args[@]}" open "$@" 2>/dev/null && return 0
    sleep 1
  done
  env "${unset_args[@]}" open "$@"
}
