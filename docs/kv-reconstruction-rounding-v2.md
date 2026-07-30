# Dynamic MLA KV reconstruction rounding and ABI v2

Date: 2026-07-30

Status: design candidate; adversarial review required before implementation

GPU authorization or evidence: none

## Decision

GLMAXX will use one explicit two-rounding reconstruction for dynamic MLA
NoPE values and advertise it under a new semantic ABI:

```text
nvfp4_ds_mla:fp8-rope-368:dynamic-token-v2
```

For stored E2M1 code `q`, E4M3 group-scale code `gq`, and stored FP32 outer
scale `s_t`, define:

```text
g = RN32(decode_e4m3(gq) * s_t)
x_hat = RN32(decode_e2m1(q) * g)
```

`RN32` is one IEEE-754 binary32 round-to-nearest, ties-to-even operation.
The two products are separate operations. Reassociation, contraction,
extended-precision retention, fast math, and flush-to-zero are forbidden.

The encoder must use the same rounded `g` as the divisor when selecting the
E2M1 code:

```text
gq = encode_e4m3(RN32(RN32(group_amax / 6) / s_t))
g = RN32(decode_e4m3(gq) * s_t)
q = encode_e2m1(RN32(x / g))
```

The existing zero-group rule remains: `gq = 0x00`, every group nibble is
positive zero, and no division by zero occurs. The existing finite,
canonical-padding, and code validation occurs before successful
publication.

RoPE and sparse-indexer reconstruction remain one explicitly rounded
binary32 multiply:

```text
r_hat = RN32(decode_e4m3(rq) * rope_scale)
k_hat = RN32(decode_e4m3(kq) * indexer_scale)
```

This decision selects the association already present in the current Rust
encoder and decoder. It does not claim that the current ABI advertisement is
correct.

## Why the ABI must change

The pre-hardening Rust reader evaluated NoPE reconstruction as:

```text
x_v1 = RN32(RN32(decode_e2m1(q) * decode_e4m3(gq)) * s_t)
```

The finite-reconstruction hardening changed it to:

```text
x_v2 = RN32(decode_e2m1(q) *
            RN32(decode_e4m3(gq) * s_t))
```

The two expressions are not bit-equivalent. The prior review independently
found ordinary writer-produced and crafted finite records whose output
differs by one binary32 ULP. The current proof's statement that rounding did
not change is therefore false.

Engine specification section 19 and format specification section 24 require
every record-interpretation change to use a new KV ABI. The 368-byte physical
layout may remain identical; semantic identity may not.

The v2 choice matches the scale actually used by the writer to normalize
each source value. It therefore gives encoder and decoder one bit-exact
definition rather than allowing the decoder to reconstruct through a
different intermediate. This is a consistency decision, not a model-quality
claim. MTP0 logits and downstream quality still require the normal
per-position gates before serving acceptance.

## Exact distinguishing vector

The CPU proof must include this record fragment:

```text
q = 0x03                         // decode_e2m1 = 1.5
gq = 0x03                        // decode_e4m3 = 0.005859375
s_t bits = 0x3d000001            // 0.0312500037252903
```

The required v2 intermediates and result are:

```text
g bits = 0x39400002              // RN32(0.005859375 * s_t)
x_v2 bits = 0x39900002
```

The forbidden left-associated result is:

```text
x_v1 bits = 0x39900001
```

An exact test must require the v2 bits and mutation-test the v1 association.
A tolerance comparison is insufficient.

## Serialization and namespace boundary

V2 retains:

- the 368-byte target/draft KV physical record;
- every byte offset and zero-padding rule;
- E2M1/E4M3 code tables and saturation;
- FP32 little-endian scale fields;
- target/draft record roles; and
- the HBM `[layer][local_page][64][368]` geometry.

V2 changes:

- NoPE reconstruction association;
- the numerical semantic ABI string;
- target and draft KV ABI hashes;
- every prefix content namespace containing either hash;
- the operation-manifest `draft_kv_record` identity; and
- any capability digest that binds the numerical decoder.

The target-indexer and combined draft-sidecar container ABIs must bind the v2
KV identity even though their physical envelope does not change.

GLMAXX is target-only and need not carry a production v1 decoder. At startup,
a v2 runtime must reject v1 KV records, v1 namespace entries, and a
v1-advertising operation manifest. Existing v1 prefix-cache estates are cold
misses, not candidates for silent reinterpretation.

An optional offline migration may copy a fully authenticated physical record
into a newly published v2 namespace only if it:

1. validates the complete v1 record and provenance;
2. decodes it with the historical v1 association;
3. decodes the same bytes with v2 and records that semantic change;
4. publishes new target/indexer/draft generation metadata under v2 hashes;
5. never mutates or relabels the old record in place; and
6. does not claim output equivalence.

No migration is required for engine bring-up. Deleting or relabelling a v1
estate is not authorized by this design.

## CPU implementation gate after review

Only after this design is accepted, one coordinated implementation must:

1. amend engine and format specifications to define the exact RN32 order and
   advertise `dynamic-token-v2`;
2. update the operation manifest and Rust reference manifest;
3. define one Rust constant for the v2 identity and remove duplicated string
   literals;
4. preserve the current group-scale-first Rust arithmetic without allowing
   compiler contraction or reassociation;
5. add the exact distinguishing vector above;
6. exhaust every finite E2M1/E4M3 code pair over adversarial FP32 outer-scale
   classes and require finite results or the existing fail-closed error;
7. mutation-test left association, missing intermediate-finite checks,
   fast-math-equivalent reassociation, v1 manifest identity, and v1 namespace
   reuse;
8. prove deterministic writer bytes are unchanged for a pinned finite corpus
   while the semantic ABI/hash changes;
9. preserve per-record finite/canonical validation;
10. prove target and draft namespaces change together;
11. pin CPU compiler flags and exact output digests; and
12. issue a separate implementation handoff before any CUDA implementation
    or model claim.

The subsequent SM120 kernel must use explicit round-to-nearest operations or
an independently proven instruction sequence. Its gate compares exact FP32
bits against the v2 CPU oracle at actual GLM-5.2 shapes. Model quality and
performance remain later, separate gates.

## Acceptance boundary

Acceptance of this design establishes only:

- one unambiguous CPU/kernel numerical target;
- the need for a semantic ABI-v2 and cold namespace boundary;
- identical physical record geometry; and
- the required CPU proof and mutation matrix.

It does not accept:

- the current code as v2-complete;
- silent v1 cache reuse or migration;
- CUDA math, SM120 evidence, attention correctness, model logits, KLD,
  retrieval, 1M execution, capacity, or performance;
- K01, K02, K04, Q01, or Q04 beyond their existing evidence; or
- cn4 access.

## Required adversarial review

The reviewer must independently:

1. reproduce at least one finite v1/v2 one-ULP divergence;
2. verify the exact distinguishing vector and both output bit patterns;
3. determine whether the v2 encoder and decoder use the same rounded group
   scale;
4. determine whether any permitted compiler contraction/reassociation could
   erase the defined intermediate;
5. verify that unchanged bytes plus changed interpretation requires a new
   ABI under both specifications;
6. trace every manifest, namespace, target/draft, indexer, sidecar, and
   capability identity that must change;
7. verify that a v2 runtime cannot silently restore v1 cache content;
8. assess whether the optional migration boundary is fail-closed and makes
   no equivalence claim;
9. verify that the physical layout and capacity arithmetic remain unchanged;
10. determine whether the proposed CPU proof distinguishes left association
    and stale-v1 identity;
11. identify every missing mutation needed to prevent semantic drift; and
12. verify every exclusion and the absence of a GPU/model-quality claim.

Withhold acceptance for ambiguous rounding, an incorrect fixed vector,
unchanged semantic identity, partial target/draft namespace migration,
silent v1 reuse, physical-layout drift, a nondistinguishing proof plan, or
any quality/GPU overstatement.
