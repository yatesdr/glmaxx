# Asynchronous restore identity CPU proof v1

Date: 2026-07-29

Implementation commit:
`4b60040566d530ddd12d8a72e10adb9d0fcdb22f`

Status: CPU/reference correction passed; independent review pending

GPU claim: none

## Defect and invariant

`ResidencyManager::begin_restore` previously changed a page from `Nvme` to
`Restoring` without retaining the request identity. `complete_restore`
accepted any result whose record selected the same page key and whose
namespace and generation matched. It did not require:

- the request ID returned by `begin_restore`;
- the requested logical page ordinal;
- the complete durable `TierRecord`; or
- a valid nonzero request and rank/ordinal ownership tuple before mutation.

This allowed a stale, swapped, or altered asynchronous result to adopt bytes
into the wrong pending operation.

Each restoring entry now retains:

```text
(request_id, page_ordinal)
```

`begin_restore` rejects zero request IDs, ranks outside TP4, and a worker rank
that does not own the page ordinal before changing residency. Completion
requires exact request ID, exact ordinal, and equality of the complete
restored `TierRecord` with the registered record. A mismatch returns
`ResidencyError::Stale` and leaves the entry in `Restoring`, so the correct
completion can still arrive or the caller can abort it.

The pending identity is cleared only by successful completion or explicit
abort. Registration and ordinary HBM/DRAM/NVMe entries carry no pending
identity.

## CPU proof

The new test proves:

- request ID zero is rejected without changing `Nvme` residency;
- a rank that does not own the ordinal is rejected without mutation;
- a wrong completion request ID is rejected;
- a wrong completion ordinal is rejected;
- a same-key record with a changed generation is rejected;
- all failed completions preserve `Restoring`; and
- the original exact completion is subsequently adopted into HBM.

The full local gate passed 232 Rust tests with zero failures, formatting,
workspace Clippy with warnings denied, CUDA FFI type checks, every
deterministic CPU proof command, and all 33 then-present handoff provenance
proofs.

Commands:

```text
cargo fmt --check
cargo test -p glm-cache
cargo clippy -p glm-cache --all-targets -- -D warnings
scripts/local-checks.sh
```

Relevant hashes:

```text
crates/glm-cache/src/residency.rs
74e7dd8077d7ce1db082b6b2501debfcf07d39f0c444e5e355bdb5385ac29770

crates/glm-cache/src/store.rs
d37a1400dc0c393b26c121f72694945bef78c28eda29796abf41a2ed713a17ac

crates/glm-serving/src/cache.rs
786c7c7e5ce2f417749a78e8c48aa8a7d0a5cb617e0883e960a8e7c17d781720

docs/cache-lifecycle-proof-v1.md
11ad4936fea7cd0887e660911f50778d5b0918c21a6cebaca1a98a244b2e2de1

spec/engine-v0.md
efaa6dcb4da3e6f40032c61d472a0f548920a3e87642efac315da2771b7df86a
```

The external tokenizer proof was skipped because
`GLMAXX_TOKENIZER_DIR` was unset; its fixture and implementation did not
change. No CUDA compiler, GPU, real HBM transfer, direct I/O, or NVMe
performance path was used. This proof does not authorize cn4 or establish
device cache correctness, model quality, long-context capacity, or
performance.
