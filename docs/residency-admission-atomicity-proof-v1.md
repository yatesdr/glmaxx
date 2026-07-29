# HBM residency admission atomicity CPU proof v1

Date: 2026-07-29

Implementation candidate:
`94f8d572668ec84d1f77ec2b6949a6bcca3b1e15`

Core planner commit:
`012914e1435e13246c0064815789ba6fea2de003`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defect and invariant

`ResidencyManager` previously made room for an HBM admission by demoting one
LRU victim at a time. If the victims selected so far were insufficient and
all remaining HBM pages were pinned, the operation returned
`ResidencyError::Pinned` after it had already moved unrelated pages to DRAM
or NVMe and changed the HBM/DRAM byte counters.

An HBM admission is now split into two phases:

1. `plan_hbm_admission` validates every byte calculation and selects the
   complete deterministic victim set without mutating manager state.
2. The target transition, complete victim plan, final byte counters, and
   logical clock are committed only after the plan succeeds.

The plan sorts eligible victims by `(last_touch, page_key)`, excludes the
admission target, never selects a pinned page, and records each victim's DRAM
or NVMe destination. If capacity cannot be satisfied, the manager returns an
error with the target, every resident page, restored bytes, counters, pins,
and logical clock unchanged.

DRAM-to-HBM promotion now accounts for the promoted page's DRAM release
before placing victims. This keeps the plan bounded without an unnecessary
NVMe spill. The deterministic cache-lifecycle proof was updated to assert
the corrected posture and corrupt the page that is actually resident on
NVMe.

`pin_hbm` also preflights state, pin-count overflow, and logical-clock
overflow before mutation.

## CPU proof

The failure-path test constructs an HBM arena holding two ordinary
target-plus-draft pages. One page is pinned, and an incoming MTP page is
larger than either resident page, so admitting it would require both
residents to leave HBM. The test proves:

- the unpinned first victim is not demoted before the later pinned-capacity
  failure is known;
- the pinned page remains in HBM;
- the incoming page remains in `Restoring`;
- HBM and DRAM counters are unchanged; and
- the pending restore remains abortable.

The paired success test uses the same two-victim geometry without the pin and
proves that both deterministic victims move to DRAM, the larger incoming
page becomes HBM-resident, and the final HBM/DRAM counters exactly match the
committed plan. The failure test distinguishes the old incremental
implementation; the success test exercises the multi-victim commit path.

The full local gate passed 234 Rust tests with zero failures, workspace
formatting, workspace Clippy with warnings denied, CUDA FFI type checks,
every deterministic CPU proof command, and all 34 then-present handoff
provenance proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo test -p glm-serving
cargo test -p glm-cli
cargo clippy -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
cd15cbbcf1031adb1fc73e5416fbf5d5149ff87096f193c8ad1b0709417f9629

crates/glm-cache/src/tier.rs
c31b07d7f9054f3d51bc5d24c2c414b6c9a134d88f042502bc0f82e29cad500f

crates/glm-cli/src/cache_proof.rs
3371395bb723d2ec092c16cfd28bcb25b54ca1e38fc2096dff471941b2ac9358

fixtures/cache-lifecycle-proof-v1.json
8d75a281e127f669f52065c7ca2fa0945a4d090e3624f17f857410122dde0dfc

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

scripts/local-checks.sh
839ec27e61aff8249ffa5b586621e6a1fa316221dd50eddf3ee467d096a1d18f

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, real HBM allocation or transfer, direct I/O,
io_uring, or NVMe performance path was used. This proof does not authorize
cn4 or establish device cache correctness, model quality, 1M-context
capacity, or serving performance.
