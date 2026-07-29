# Fable handoff: checkpoint load transaction v1

Date: 2026-07-29

Status: superseded before review by
`docs/fable-checkpoint-load-transaction-v1-r2-handoff.md`; do not issue this
handoff's token

GPU authorization conveyed by this handoff: none

Review candidate commit:
`737603b4df40605ae47500c5ff9aec3a6b116293`

Requested acceptance token, only if every blocker and major is resolved:
`checkpoint-load-transaction-v1-accepted`

## Provenance

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/checkpoint-load-transaction-v1.md` | `03855dcb090f7ba7710b190580440b74c47147c777904b0926318be97e04f3da` |
| `spec/engine-v0.md` | `efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a` |
| `spec/format-v0.md` | `619a3923c18f43edb23ca9de44b51b84c6d2f6432915908db5fa3a2e0e7cf45a` |
| `docs/checkpoint-ingest.md` | `81408c1da077e7bbe297d94214f4e0497403ec3fed0b789b0da74c300e31d5c6` |
| `docs/sm120-rank-runtime.md` | `19638590ee3b42da32bfab7673986c26488da064649c635df895700838da5624` |
| `crates/glm-format/src/native_reader.rs` | `ae3579593713d35f633fadd1fe326db0ba8bae6ffe3644643e73b3321a6a0b4c` |
| `crates/glm-engine/src/startup.rs` | `9634f120a2e01f21aaa5778954053d9a06f1e8d2af6c5abe1f9c6e4cbbd31e87` |
| `crates/glm-engine/src/worker.rs` | `400c7c22f2c74d3f386fefe2d144da3437f27e6c99ce9f2d4bbf87ffe98fe437` |
| `crates/glm-engine/src/memory.rs` | `3a50581a8a60970a92ccf5a2c0e83c23d25ad975f1124c2332e9a2e646dbc837` |
| `crates/glm-engine/src/weight.rs` | `d658cefefc17757a28258bafd0e13f5309e8adcbf2b30c4d2bdc97be9899ca19` |
| `profiles/profile-budget-v0.json` | `cdbe4eaad9465181b2ba60b3656fe5207eee54467abfbf8d9bc398c3e68c23e0` |
| `manifests/glm52-operation-v1.json` | `8a5f5488bb31640712d5bd2d39fe70de3eab65a87759bc8bb186646a53123da6` |
| `docs/production-punchlist.md` | `f568257b195ca3bd354860139b6a2ba0a06007b28c0337e5c29d1aec3d132add` |
| `crates/glm-cli/src/review.rs` | `d2c2d2756b94df8fb5555f578e7c907bef7c09b7b10fb3f310f45566f73c1c45` |
| `docs/review-provenance-verifier-v1.md` | `c4be2415ad0b13cea7fc154ce10c7aea839bd47b57af3e42fa6f329b92f3cb4e` |

Run this fail-closed check before reviewing:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof docs/fable-checkpoint-load-transaction-v1-handoff.md
```

Hash every input at review start and finish. Review the exact candidate
commit in a separate worktree if `main` advances. The candidate contains no
load-transaction implementation, CUDA upload sink, device allocation,
checkpoint smoke, full-rank proof, performance result, or GPU evidence.

## Candidate decisions

The candidate:

- treats the normative engine startup order as authoritative and explicitly
  rejects the current mock's memory-after-weight-load order;
- freezes a binary process-common load plan, four rank entries, per-tensor
  device layout, prepared-rank receipt, and four-rank receipt digest;
- binds model, tokenizer, chat template, operation manifest, tensor catalog,
  policy, profile budget, memory plan, codec capabilities, ABI, files, device
  identities, and physical arena layouts before allocation;
- uses type-state quarantined arenas that cannot reach a `RankExecutor`
  before four-rank adoption;
- defines the borrowed reader-buffer versus owned pinned-ring lifetime needed
  for safe asynchronous host-to-device copies;
- requires `FULL_SHA256` on first load, complete async error/drain handling,
  exact byte counts, and no rank-local fallback;
- defines process-atomic adoption without claiming simultaneous cross-device
  execution; and
- keeps full residency blocked on EXL3 device acceptance, a complete reviewed
  profile budget, quality gates, and measured capacity.

## Requested adversarial questions

1. Does the proposed startup ordering match engine v0 exactly? Should the
   current Rust mock be replaced outright or versioned as a non-production
   state machine?
2. Recompute every offset and total in the 416-byte plan header, 216-byte rank
   entry, 64-byte tensor entry, and 256-byte prepared receipt. Are any fields
   ambiguous, under-bound, redundant in a dangerous way, or missing?
3. Is the domain-separated plan and receipt hashing complete and
   non-circular? Can two materially different load routes or arena mappings
   share an identity?
4. Are model config, tokenizer bundle, chat template, operation manifest,
   tensor catalog, profile budget, policy, ABI, capability, memory, device,
   and rank-file identities sufficient? Identify any additional source or
   runtime identity required before `WEIGHTS_LOADED`.
5. Is `tensor_contract_sha256` a trustworthy common catalog only if the
   loader parses and independently recomputes it from all four canonical
   manifests? Specify the exact fail-closed checks required.
6. Does the physical tensor entry contain enough information to prove exact
   metadata, primary, and auxiliary destination coverage without overlap,
   holes that matter, uninitialized bytes, or descriptor/layout drift?
7. Can file padding safely be verified but omitted from HBM? Must device arena
   alignment or zeroed gaps be included in the layout hash and byte proof?
8. Is it memory-safe for the sink to copy the borrowed 8 MiB reader chunk
   into an owned pinned ring, enqueue an asynchronous copy, and return?
   Identify any callback, event, ring-reuse, short-chunk, or shutdown lifetime
   race.
9. Is a minimum two-slot ring sufficient to overlap host copying and PCIe
   transfer without creating a hidden unbounded queue? Should slot count and
   size be fixed rather than merely plan-bound?
10. Since host SHA-256 validates bytes before/while they are uploaded but not
    HBM contents, is CUDA completion/error checking sufficient for the
    correctness gate? If not, require an exact device digest, copy-back, or
    other end-to-end device-memory proof and define its coverage.
11. Can a late rank-3 hash failure, asynchronous error, timeout, or thread
    exit leave ranks 0–2 prepared or adopted in a way a later startup stage
    can observe?
12. Does `Drop` synchronization make abort safe for in-flight copies and
    events? How must cleanup behave if synchronization itself reports a CUDA
    error, the rank thread panics, or a timeout fires while work is still
    running?
13. Is the prepare/adopt split truly process-atomic given that ranks move
    their handles one at a time? Is “scheduler closed until HEALTHY plus
    terminal teardown on partial adoption” a sufficient visibility proof?
14. What exact command/acknowledgment changes are required in
    `Tp4WorkerPool` so CUDA contexts and allocations remain owner-thread-only
    and no coordinator thread touches a device pointer?
15. Does `PreparedRankReceipt.v1` bind enough evidence to prevent a receipt
    from a prior allocation generation, device, file, or load attempt from
    being replayed?
16. Should timing values be outside the fixed receipt and bound only through
    `verification_evidence_sha256`? Define the canonical evidence artifact
    required to make that hash reproducible.
17. Does the resource accounting keep HBM, pageable host control memory,
    pinned host staging, file cache, temporary device verification, and
    allocator padding separate enough to prevent a false fit result?
18. Can a blocked `capacity-exl3` profile or laboratory subset ever reach a
    serving startup state through profile-byte, manifest, or budget
    substitution? Re-derive the necessary gates.
19. Is process-wide restart the only safe fallback after allocation begins?
    Identify any retry that could be allowed without violating identical
    four-rank route selection.
20. Is the FS-verity route correctly unavailable until its independent
    provenance and implementation gate exists?
21. Does the required CPU/mock matrix cover every type-state, byte-accounting,
    late-failure, partial-adoption, ownership, and cleanup boundary needed
    before CUDA implementation?
22. Does this design conflict with the format or engine specification, the
    current weight-policy/memory types, or the persistent-worker ownership
    contract? Identify every required spec or ABI amendment.

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`. Withhold
the token unless every blocker and major is resolved. State separately
whether:

- the startup ordering correction is accepted;
- the binary plan and receipt encodings are accepted;
- the pinned-ring asynchronous lifetime proof is accepted;
- host verification plus CUDA completion is sufficient or a device-content
  proof is mandatory;
- process-atomic four-rank adoption is accepted;
- the memory/resource accounting boundary is accepted;
- CPU/mock implementation may begin;
- a CUDA upload implementation remains blocked;
- full-checkpoint load remains blocked; and
- no cn4 access or GPU launch is authorized by the verdict.
