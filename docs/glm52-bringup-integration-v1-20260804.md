# GLM-5.2 bring-up integration v1

Date: 2026-08-04

Status: clean integration base; no full-model or throughput claim

Branch `integration/glm52-bringup-v1` combines two independently qualified
lines on top of `origin/main` at `7e8782b`:

- real-checkpoint SM120 EXL3 K3 target and recurrent-draft projection controls
  from `qualification/exl3-real-k3-v1` at `7df1a80`; and
- pinned Local Inference Lab request, stream-usage, EOS, authentication, and
  model-discovery compatibility from `feature/decode-bench-api-v1` at
  `6b4a3cf`.

The integrated candidate before this record is `73217d4`. The complete local
gate passes 427 Rust tests, Clippy, deterministic CPU proofs, 152 handoff
checks, and 39 accepted results out of 134 configured. CUDA compilation was
not attempted on the local macOS host.

The SM120 evidence remains external and immutable:

- target K3: `/home/derek/glmaxx/evidence/20260804T052506Z-exl3-real-k3-e8e4593-r1`,
  manifest SHA-256
  `e41a282967c2ce747d08888fe3b59c6a5e3c5eec936a1c91cdea7e38c7d4ad61`;
- recurrent-draft K3:
  `/home/derek/glmaxx/evidence/20260804T054043Z-exl3-draft-k3-99ba366-r1`,
  manifest SHA-256
  `812aa10a7989350df90537cb06961b78abe9f004ed7098dfdb8acba1e8ec27f3`.

These paths prove scalar K3 projection correctness only. The mixed-K r2,
safetensors `total_size` r2, production rank-executor r2, and target-layer r2
acceptance tokens remain absent. K4, checkpoint admission, TP4 layer replay,
full-model execution, quality, capacity, MTP3, and performance remain closed
until their gates pass.

The next accepted change lands on this branch in order: mixed K3/K4 CPU/source
proof, specialized SM120 K4 control, persistent-rank target-layer integration,
then real TP4 checkpoint smoke.
