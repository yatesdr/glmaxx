# Target-layer r2 preflight erratum

Date: 2026-08-04

Status: the earlier preflight's no-false-premise conclusion is withdrawn;
candidate and handoff corrected for a fresh adversarial review

`docs/target-layer-r2-preflight-20260804.md` correctly records the tests,
hashes, source derivations, and arithmetic performed against candidate
`d4817ff9ff7eec09c74e98a99db5c27690286013`. It incorrectly concluded that
the handoff contained no false premise.

The r2 lifetime table serializes logical slot class, phase mask, writer and
last-reader stages, reuse-eligibility class, and flags. `GraphProfile.v2`
binds that digest plus the retained v1 graph-profile hash, whose only scratch
field is the aggregate `maximum_scratch_bytes`. Neither record serializes a
physical class arena, offset, or capacity. The old handoff nevertheless asked
the reviewer to reject every prohibited alias and *undersized graph slot* and
to test `GraphProfile.v2` anti-alias behavior. The latter two requirements
were not executable from the candidate bytes.

The corrected candidate now makes the boundary explicit: logical alias
eligibility can be reviewed and implemented with distinct CPU-owned storage;
physical reuse, class capacity, one-byte-short rejection, and CUDA graph
capture remain closed until a later byte-specified and hash-bound physical
span ABI. The handoff asks the reviewer to verify that boundary rather than
attest absent machinery. Historical preflight evidence remains unchanged and
must not be used as adversarial acceptance.
