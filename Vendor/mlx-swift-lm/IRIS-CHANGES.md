# Iris's copy of mlx-swift-lm

A copy of [ml-explore/mlx-swift-lm](https://github.com/ml-explore/mlx-swift-lm) at commit
`c8dabc63e7018d0b1567b2171f8df5dd8a334a55` (MIT licence, `LICENSE`), with the changes below.
Iris pinned that commit before: no release yet carries the Gemma 4 E-series MTP drafter (#383)
and the fix for dropped speculative tokens (#731).

Left out of the copy: `Tests`, `IntegrationTesting`, `scripts`, `skills`, `tools`, the GitHub
and contributor files, and the test targets in `Package.swift`. Nothing Iris builds uses them.

## Changes

**Gemma 4 for text only, with less memory** (`Libraries/MLXVLM/Models/Gemma4TextOnly.swift`,
and the lines marked `Iris:` in `Libraries/MLXVLM/Models/Gemma4.swift`). Off unless
`Gemma4TextOnly.wordTableFile` is set before a load; Iris sets it for its own trained models.

- The per-layer word table (`embed_tokens_per_layer`: 1.3 GB in E2B, 1.6 GB in E4B) isn't held
  in memory. Each forward pass reads the rows its tokens need from the model's
  `model.safetensors` with `pread` and dequantizes them, the same 4-bit rows dequantized the
  same way. Group size and bits come from the file's shapes. If the file doesn't hold the table
  as expected, the stock embedding is built instead.
- The picture encoder is built with no layers, and its layers' weights are left out when the
  checkpoint is loaded. Iris never sends pictures.

Measured on Iris's models (evals, on the developer's Mac): word-for-word the same answers as the
stock model on 732 test drafts, at the same speed, with 1.6 to 1.9 GB less memory.

**Faster Gemma 4 text inference and hybrid speculative decoding** (marked `Iris:` across
`Libraries/MLXVLM/Models/{Gemma4,Gemma4TextOnly,Gemma4Assistant}.swift` and
`Libraries/MLXLMCommon/{MTPSpeculativeTokenIterator,MTPDrafterModel,Evaluate,KVCache,KVCacheRound,SuppressTokens,Models/Gemma4Encoders}.swift`):

- **Compiled fusions and fused projections (`Gemma4.swift`, `Gemma4Encoders.swift`)**:
  - Compiled `residual + rmsNorm(x)` / `(residual + rmsNorm(x)) * scale` / `geluApproximate(gate) * other`
    fragments (matching `MLXLLM`'s `Gemma4Text.swift`), plus static-shape compiled FFN + per-layer
    input blocks on fixed speculative verify widths (`L <= 6` or `L == 32`).
  - Coalesced `gate_proj + up_proj` (both quantized and dense BF16 for the drafter) and
    `q_proj + k_proj (+ v_proj)` into single matmuls sharing backing storage (`prepareFusedProjections()`).
  - Prefill projects `lm_head` only for `hidden[:, -1:, :]` (`lastTokenLogitsOnly`), skips `lm_head`
    on intermediate chunks and prefix-cache prefill (`Gemma4.prefillOnly`), and emits MTP drafter
    state directly from prefill (eliminating a redundant 42-layer prime forward pass per request).
  - Greedy verification skips the strictly monotonic FP32 logit softcap (`mtpSkipLogitSoftcapKey`).
- **On-disk per-layer word table row cache (`Gemma4TextOnly.swift`)**:
  - Bounded 2,048-token quantized row cache (~12 MB cap) avoids repeated `pread` syscalls on warm
    tokens while keeping the 1.3–1.6 GB table off RAM, copying each row immediately after slot
    resolution so later misses in the same lookup cannot evict an earlier hit before copy, with
    file-bounds/header validation and task-local `withWordTableFile` scoping across concurrent loads.
- **MTP drafter & verification fast paths (`Gemma4Assistant.swift`, `SuppressTokens.swift`, `MTPSpeculativeTokenIterator.swift`, `Evaluate.swift`)**:
  - `Gemma4AssistantMaskedEmbedder.sampleGreedy` runs greedy argmax directly over the ~4K selected
    canonical candidates inside a compiled drafter head instead of scattering to 262K vocab; drafter
    attention passes `.none` mask instead of allocating all-zero bidirectional masks.
  - `SuppressTokensProcessor` clamps config token IDs to `0...Int32.max`; `switchToPassthrough`
    writes diagnostics to `stderr` (never `stdout`); `mtpSkipLogitSoftcapKey` is explicitly cleared
    and reset; `generateTokensTask` exposes the MTP worker `Task` so callers can await unwind on cancel.
  - Batched greedy verification computes all `numDraft + 1` argmax tokens in one GPU `eval()`.
  - Adaptive Hybrid Prompt-Lookup Drafting (`promptLookupTokens`, `promptLookupMaxDraft`) proposes
    up to 31 verbatim tokens (`L = 32` Apple Silicon NAX tile) on CPU during Fix mode and falls
    back to the MTP drafter with cooldown backoff on typo clusters.
- **In-place multi-token sliding KV cache updates (`KVCache.swift`, `KVCacheRound.swift`)**:
  - Unwrapped `RotatingKVCache` entries write multi-token rounds in place (`AppendOnlyRoundStrategy`)
    instead of concatenating full KV history twice per layer on every speculative round.

## Updating

Copy the new upstream commit over `Libraries` and `Package.swift` (dropping the test targets
again), then reapply the changes: `Gemma4TextOnly.swift` is self-contained; across the remaining
files listed above, search for `Iris:`.
