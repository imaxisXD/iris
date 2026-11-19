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

## Updating

Copy the new upstream commit over `Libraries` and `Package.swift` (dropping the test targets
again), then reapply the change: `Gemma4TextOnly.swift` is self-contained; in `Gemma4.swift`,
search for `Iris:`.
