# Cache arena budget v2 r2 CPU proof

Date: 2026-07-30

Status: corrective CPU implementation passed locally; adversarial review
required

GPU claim: none

## Scope

Fable's review of candidate `c33648a` found zero blockers and zero majors,
independently rederived the 266,688-slot/4,167-page MTP6 arena, and approved
the static arithmetic for serving integration. It identified seven minor
findings and two questions.

This correction closes the six static-planner/test minors. The seventh minor,
clone/full-scan page-table mutation, is deliberately not hidden here; it is
owned by the separate fixed-page transaction r2 design and remains
unimplemented pending its review.

## Implemented corrections

### One source for cache byte terms

`ProfileBudgetArtifact::validate` no longer compares against four hand-copied
byte literals. `capacity_exl3_cache_terms` derives all four from:

```text
MIN_LOCAL_CAPACITY_TOKENS
MIN_PAGE_SLACK_SLOTS_PER_RANK
MIN_MTP_TENTATIVE_SLOTS_PER_RANK
TARGET_LAYERS
INDEXER_GROUPS
DRAFT_INDEXER_GROUPS
KV_RECORD_BYTES
INDEXER_RECORD_BYTES
PAGE_TOKENS
```

The existing checked multiplications remain the only byte arithmetic. A
regression requires the derived values to remain exactly:

```text
target KV          7,655,012,352
target indexer       739,259,136
draft KV              98,141,184
draft indexer         35,202,816
```

### Symmetric rank arenas

`plan_system_memory` now rejects any rank disagreement in committed, page
slack, tentative, or rounded target/draft slots with
`MemoryPlanError::ArenaMismatch`. It no longer silently takes per-field
minima and charges larger ranks for unreachable capacity.

The process still allows rank-specific measured HBM, non-cache terms, and
headroom. Only the page-table arena shape is process-common, as required by
TP4/DCP4 execution.

### Constructible page-table configuration

`glm-cache` now exports the one authoritative
`MAXIMUM_PHYSICAL_PAGES_PER_RANK`. `SequencePageTable::new` and
`plan_system_memory` consume the same constant.

The system planner rejects:

- zero target pages;
- target pages above 1,048,576;
- draft pages above target pages; and
- non-page-aligned or overflowing slot counts.

Therefore a successful `SystemMemoryPlan.v2` always produces a
`PageTableConfig` accepted by the retained table constructor.

### Pending budget floor

Every profile rank, including a pending non-convertible rank, now requires:

```text
required_bytes <= planned_usable_hbm_floor_bytes
                 <= observed_pre_context_free_bytes
```

Pending status and `conversion_allowed=false` are no longer the only reason
an arithmetically impossible floor remains harmless.

### Exact physical-margin regressions

The MTP0 and MTP6 constants document their reviewed margins. A permanent
regression constructs 64 sequences, commits 16,384 tokens to each, then
reserves one spill token for each sequence. It observes:

```text
target pages by rank = [4,160, 4,096, 4,096, 4,096]
draft  pages by rank = [4,160, 4,096, 4,096, 4,096]  // MTP6-capable
draft  pages by rank = [0, 0, 0, 0]                  // MTP0
```

The exact reviewed arenas therefore retain:

```text
MTP6: 4,167 - 4,160 = 7 pages
MTP0: 4,161 - 4,160 = 1 page
```

This is a conservative physical-pressure construction. It does not claim
that a production admission controller may publish committed positions above
its separately configured global logical-token quota.

## Review questions disposition

The global logical admitted-token/C64 policy remains a serving admission and
page-transaction responsibility. The current physical table fails capacity
atomically, and the scheduler caps a step at C64, but this proof does not
claim that the planned tenant/global quota ledger is implemented. C03 and S06
remain open/review for that reason.

The tentative-commit relocation question is already corrected in the current
table: accepted target/draft pages commit in place, while only rejected pages
enter generation-bound quarantine. The dedicated quarantine proof and review
remain the authority for that behavior.

The retained table still clones for transactional rollback and the retained
mirror still scans complete mappings. Those types remain correctness oracles,
not the production hot path. `docs/fixed-page-transaction-v1-r2.md` owns the
bounded suffix-only replacement and its required touched-work mutations.

## Verification

The focused gate passed:

```text
cargo test --offline -p glm-engine memory::tests -- --nocapture
  14 passed

cargo test --offline -p glm-cache sequence::tests -- --nocapture
  12 passed

cargo clippy --offline -p glm-engine -p glm-cache --all-targets -- -D warnings
  passed
```

`scripts/local-checks.sh` then passed with:

- 378 Rust tests and zero failures;
- workspace formatting and Clippy with warnings denied;
- CUDA FFI host checks;
- deterministic format/cache/engine/serving fixtures;
- review provenance for all 107 then-present handoffs;
- the external tokenizer proof skipped because its configured directory was
  absent; and
- CUDA compilation skipped because this host has no `nvcc`.

No CUDA context, GPU, remote host, model weight, or checkpoint was used.

## Acceptance boundary

Acceptance covers only:

- derived static cache byte validation;
- symmetric TP4 cache arena planning;
- constructible page-table bounds;
- pending budget-floor validation;
- exact MTP0/MTP6 physical-margin tests; and
- the named CPU checks.

It does not accept the retained clone table or mirror as a production hot
path, implement the global/tenant quota ledger, accept measured profile fit,
authorize conversion, or establish CUDA/KV payload/model/quality/capacity/
performance evidence. It does not authorize cn4 access.
