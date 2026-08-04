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

The exact integrated commit `c30975a40084c2f8945faad0db5b1aa52fb0aacf`
was then rebuilt in the pinned cn4 container for SM120 and passed the complete
427-test Rust gate plus both real-checkpoint K3 projection regressions. Each
regression covered gate, up, and down projections on TP ranks 0 and 3 at
M=1/2/4/8, for 24 cases with exact CPU/GPU output hashes and deterministic
repeats. The target-layer run also required and passed its K4 fail-closed
negative control.

The SM120 evidence remains external and immutable:

- target K3: `/home/derek/glmaxx/evidence/20260804T052506Z-exl3-real-k3-e8e4593-r1`,
  manifest SHA-256
  `e41a282967c2ce747d08888fe3b59c6a5e3c5eec936a1c91cdea7e38c7d4ad61`;
- recurrent-draft K3:
  `/home/derek/glmaxx/evidence/20260804T054043Z-exl3-draft-k3-99ba366-r1`,
  manifest SHA-256
  `812aa10a7989350df90537cb06961b78abe9f004ed7098dfdb8acba1e8ec27f3`.

The integration-regression records for exact commit `c30975a` are:

- target layer 3:
  `/home/derek/glmaxx/evidence/20260804T063300Z-bringup-target-k3-c30975a-r4`,
  manifest SHA-256
  `4ee2709211120c904e7d65648f17fdd38c65318cec7b9771b851c1db138492e6`,
  summary SHA-256
  `298ca10822f8a2e12a1e8466365fddef2e6f43c57ba65f1f5f548351e22a6560`;
- recurrent draft layer 78:
  `/home/derek/glmaxx/evidence/20260804T065500Z-bringup-draft-k3-c30975a-r1`,
  manifest SHA-256
  `f1de583ca285807a80e168e65b87df6408191adfa51b9e6631c4ac5d54c589dd`,
  summary SHA-256
  `15194e9f42117c9b4fa053f5484df237557913210c03f1ba5f1635ae7622fc25`.

Both integration runs used real read-only 3.25-bpw checkpoint shards and left
the remote source worktree clean. The measured route is intentionally labeled
`scalar_control_only`; its roughly 0.46 ms gate/up projection latency is a
correctness baseline, not an optimized-kernel or end-to-end throughput claim.

These paths prove scalar K3 projection correctness only. The mixed-K r2,
safetensors `total_size` r2, production rank-executor r2, and target-layer r2
acceptance tokens remain absent. K4, checkpoint admission, TP4 layer replay,
full-model execution, quality, capacity, MTP3, and performance remain closed
until their gates pass.

The next accepted change lands on this branch in order: mixed K3/K4 CPU/source
proof, specialized SM120 K4 control, persistent-rank target-layer integration,
then real TP4 checkpoint smoke.
