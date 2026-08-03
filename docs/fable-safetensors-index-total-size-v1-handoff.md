# Fable handoff: safetensors index `total_size` accounting v1

Date: 2026-08-03

Status: superseded; do not review and do not issue the v1 token

Use `docs/fable-safetensors-index-total-size-v1-r2-handoff.md`. The r2
candidate makes directory semantics, typed diagnostics, mutual exclusivity,
and concurrent-source stability explicit.

GPU authorization conveyed by this handoff: none

Do not connect to cn4 or run CUDA for this review.

Review candidate commit:
`bb8b7d6b9529b69bfaa3d9b981df7412e39bb30b`

Required result path:
`fable-safetensors-index-total-size-v1.md` at the repository root.

Requested acceptance token, only if every blocker and major is resolved:
`safetensors-index-total-size-v1-design-accepted`

## Provenance

Review the exact candidate in a detached worktree. Hash every input at review
start and finish. Withhold the token for any mismatch or incomplete input.

| Input at candidate commit | SHA-256 |
|---|---|
| `docs/safetensors-index-total-size-v1.md` | `76514cd557a16aa20aed6b9b0e9bd532424be0f69c75cb7cc305f58cafd917e6` |
| `crates/glm-format/src/safetensors.rs` | `4a7d8d4a2121a2257a5e8b7ec531c98b4b83bddb6ea140ade697088a05009594` |
| `crates/glm-cli/src/main.rs` | `04537c79fe4bcac67627483e96fcedc783702d08a16db8c10f3894964fe99afc` |
| `docs/checkpoint-ingest.md` | `186ce5985ca1adbf280a011a7692ae07780736f842c786dccb599ed8a458d07d` |
| `AGENTS.md` | `d78d69429dab43d096c49b795f24b8e00b71a6c1d7c1d535ad431c0f4ec9bf02` |

Run the complete CPU-only local gate and record its exit status:

```text
./scripts/local-checks.sh
```

## Required decisions

Independently recompute the two accounting rows from the recorded numbers and
answer:

1. Does the contract admit both observed producer conventions without a
   checkpoint-name, digest, tolerance, range, or wildcard exception?
2. Do exact shard inventory, contiguous data coverage, and checked sums prove
   that `actual_file_bytes - actual_payload_bytes` is precisely container
   prefix/header overhead?
3. Does any declared value other than the exact payload or exact complete-file
   total fail closed with all three totals diagnosable?
4. Are absent and non-`u64` metadata treated safely and unambiguously?
5. Are the public field names and reported accounting semantics non-misleading?
6. Is production content authentication kept separate and mandatory, with no
   authority granted to `.manifest_verified` or directory inventory?
7. Can the pass remain metadata-only, with no model-payload read added to cold
   start?
8. Does the CPU matrix cover boundary values, malformed inputs, overflow,
   ordering, mutation, and both real checkpoint conventions?
9. Are the provenance and vLLM-isolation claims appropriately bounded to the
   recorded discovery run?

Return findings ordered `BLOCKER`, `MAJOR`, `MINOR`, and `QUESTION`, then
answer every decision with an unqualified `YES` or `NO`. Only if every answer
is `YES`, attest the candidate and all five exact input hashes, then end with
the requested token as the only bare acceptance line.

Acceptance opens only the CPU implementation and real-checkpoint admission
proof. It does not accept any implementation, checkpoint content, GPU run,
quality result, or performance claim.
