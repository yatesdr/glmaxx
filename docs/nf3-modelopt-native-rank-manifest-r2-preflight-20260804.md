# NF3/ModelOpt native rank-manifest r2 preflight

Date: 2026-08-04

Status: independent review-readiness diagnostic; not design acceptance, CPU
implementation, checkpoint conversion, HBM allocation, or capacity evidence

## Pinned candidate

The exact handoff candidate was inspected in clean detached worktree
`/tmp/glmaxx-nf3-source-preflight-2b87` at:

```text
2b8785907c11d2b58d8c5fa7f782845fae03e3ad
```

All twelve hashes in
`docs/fable-nf3-nvfp4-native-rank-manifest-v1-r2-handoff.md` matched at the
end of this preflight. The same candidate and hash set had already passed the
complete local gate with 413 tests during the source/kernel r2 preflight. The
required reviewer artifact and exact token remain absent.

## Fresh real-source inputs

At `2026-08-04T09:53:03Z`, only three small metadata files were copied
read-only from the real cn4 hybrid checkpoint into the temporary local root
`/tmp/glmaxx-hybrid-manifest-preflight.SBEKZG`. No shard payload, CUDA context,
or GPU process was opened.

| Input | Bytes | SHA-256 |
|---|---:|---|
| `config.json` | 145,588 | `254974797e9f455716a30ab5505ba68272181b20b58a3693e54f94fb8056f3ef` |
| `model.safetensors.index.json` | 13,720,135 | `6eb773222d932418dd0530c63aca498f86ef424da2a4526ccba76b59726da234` |
| `mxfp8_tier_nokvb.json` | 10,703 | `ebcd6087180033d4512fafa5f154f4fecfbc1ee5e5051448f34859cccc4430f0` |

These are the exact contract identities. The index reports 365,968,736,768
payload bytes.

## Independent verifier

A temporary standalone Ruby verifier reimplemented the protected inventory,
TP4 rank shapes, actual tier-map expansion, source-component names, physical
native names, unsigned-byte sort, payload-plane planner, file/device metadata
planners, and MTP3 sensitivity arithmetic. It imported no GLMAXX crate code
and did not serialize a candidate native format.

```text
temporary source SHA-256
df8d3fd2b50c98517ad573e74f581d3f468dbaa7133f57aeb9a501658efc95b1

deterministic JSON output SHA-256, repetition 1
4e54c085872811c3b307034a89b8294faeaefb0d7c6a58d9222548bb8973bfae

deterministic JSON output SHA-256, repetition 2
4e54c085872811c3b307034a89b8294faeaefb0d7c6a58d9222548bb8973bfae
```

The verifier generated all source expert components from the real 75-layer
bit map plus the uniformly ModelOpt draft layer. Exact set equality against
all index names passed. A separate `jq`/`awk` derivation, independent of the
verifier's set builder, found 75/75 target layers with exactly 192 NF3 and 64
ModelOpt experts and reproduced these source suffix counts:

| Source suffix | Count |
|---|---:|
| `weight_packed` | 43,200 |
| `weight` | 15,168 |
| `weight_scale` | 58,368 |
| `weight_scale_2` | 15,168 |
| `input_scale` | 15,168 |
| non-routed protected tensors | 1,217 |

## Exact results

The real source index and independently generated contract agreed on every
name, not only aggregate counts:

```text
source tensors                         148,289
source expert components               147,072
protected source/native names            1,217
target NF3 experts                      14,400
target ModelOpt experts                  4,800
draft ModelOpt experts                     256
all routed experts                      19,456
routed native descriptors               38,912
all native descriptors                  40,129
```

The complete native physical-name stream has SHA-256
`ea83fa5090333df6b15ee976533deb9ec0ad2d573e789212dfb826a40db14e23`.
Its last routed name is exactly
`model.layers.9.mlp.experts.99.gate_up_proj.weight`, and the real bit map
selects NF3 for that expert.

The ordered payload simulation placed every nonempty plane at a 256-byte
boundary and observed zero padding gaps:

```text
protected payload                      11,959,396,352
NF3 payload                            55,207,526,400
ModelOpt payload                       26,839,351,296
payload alignment gaps                              0
device weight arena                   94,006,274,048
```

The independent metadata simulation reproduced:

```text
catalog bytes                               8,988,896
descriptor-array bytes                     10,273,024
raw file codec metadata                     7,471,104
device codec metadata                       9,961,408
device alignment padding                    2,490,304
total immutable device arenas          94,016,235,456
```

The 192-byte ModelOpt correction adds 64 raw bytes to each of 10,112
ModelOpt descriptors, or 647,168 bytes. It removes the same 64 bytes from
each inter-record device gap while the final record remains 192 bytes, so the
device metadata total is unchanged from the earlier sensitivity.

The MTP3 cache arithmetic also matched exactly:

```text
physical slots per rank                       135,424
target KV                               3,887,210,496
target indexer                            375,395,328
draft KV                                   49,836,032
draft indexer                              17,875,968
MTP3 cache arena                        4,330,317,824
remaining after immutable plus cache    3,021,189,184
older fixed sensitivity                 2,550,136,832
provisional residual                      471,052,352
```

## Finding and nonclaims

No arithmetic, inventory, ordering, source-binding, payload-gap, or metadata
contradiction was found in the pinned r2 design. The small 471,052,352-byte
residual confirms that physical capacity remains a hard gate: this preflight
does not cover final modules, collectives, graphs, simultaneous scratch,
allocator fragmentation, full checkpoint upload, cache writes, or the
required independently free 1-GiB escrow.

This record does not replace Fable's independent review. The exact
`nf3-modelopt-nvfp4-native-rank-manifest-v1-r2-design-accepted` token remains
mandatory before the checked planner/serializer implementation is opened.
