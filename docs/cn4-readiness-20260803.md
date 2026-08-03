# cn4 readiness record — 2026-08-03

Status: read-only inventory and local proof; no CUDA context or GPU work

## Source

- Design candidate: `1efdf101f42439587b38c147f8a9ff5ee8b59ba5`
- Re-review handoff commit: `781ca53`
- Corrective design:
  `docs/current-tree-review-acceptance-v3-r2.md`
- Handoff: `docs/fable-current-tree-review-acceptance-v3-r2-handoff.md`
- Handoff `review-proof` verdict: `PASS`, five of five candidate input hashes
  exact, requested token parsed exactly.

## Local proof

Command:

```text
./scripts/local-checks.sh
```

Final result: exit 0. It passed all 410 Rust tests, formatting, Clippy with
warnings denied, CUDA-FFI type checking, deterministic proof regeneration,
124 review handoffs, and the serving/cache proofs. The tokenizer fixture was
not configured and CUDA compilation was unavailable on this host, matching
the script's explicit skips.

An earlier complete-gate attempt failed once in
`cache_lifecycle_is_bounded_recoverable_and_fail_closed` with
`Store(WriterLocked)`. The exact isolated test then passed 20 consecutive
runs, and the complete gate passed on rerun. No correctness claim is based on
discarding the failed sample; retain this as a flake watch item for concurrent
proof execution.

## cn4 read-only inventory

Observed over SSH at `2026-08-03T12:35:35Z` on the authorized cn4 endpoint:

- host reported `cn4`;
- four `NVIDIA RTX PRO 6000 Blackwell Workstation Edition` devices, compute
  capability 12.0;
- memory used was `2, 2, 2, 10 MiB` of `97,887 MiB` per GPU and utilization
  was 0% on every GPU;
- no vLLM, SGLang, Ray, GLMAXX, or model Python process was present; only host
  maintenance Python processes were observed;
- `/home/derek/glmaxx` exists with separate `src`, `build`, `cache`, `deps`,
  `evidence`, `source-overlays`, and `worktrees` trees; and
- `/home/derek/glmaxx/src` was clean and detached at `e4f0290`, far behind
  current development.

Commands were limited to hostname/time, `nvidia-smi` query mode, process-name
inventory, directory listing, and Git status/log/remote inspection. The SSH
session was closed immediately afterward. No file, process, container, CUDA
state, vLLM asset, checkpoint, or evidence was changed.

## Next gate

Do not launch the profiler or update the detached cn4 source as qualifying
evidence until both the SM120 profiler package review and current-tree v3-r2
design/implementation chain are accepted and pinned. Then create a new clean
worktree under `/home/derek/glmaxx/worktrees`, keep build/cache/evidence paths
external and GLMAXX-only, rerun occupancy, and execute the accepted preflight.
