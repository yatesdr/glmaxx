# cn4 TP4 eager-module memory baseline

Date: 2026-08-03

Status: SM120 diagnostic; no capacity, kernel, checkpoint, or serving claim

## Result

Five fresh-container runs held four CUDA contexts and one nonblocking stream
per rank with `CUDA_MODULE_LOADING=EAGER`. All five result JSON files were
byte-identical:

```text
result SHA-256  05d44a0957de948ad14bce7d0cd497865d2217503067a5c6d7ee0608261fac8c
rank 0 free     101,382,881,280
rank 1 free     101,382,881,280
rank 2 free     101,382,881,280
rank 3 free     101,365,645,312
minimum free    101,365,645,312
```

The matched lazy baseline's minimum was 101,367,742,464 bytes. Eager loading
of the current linked SM120 library therefore consumes exactly 2,097,152
additional bytes on every rank. No GLMAXX device allocation other than
runtime/context state and one stream per rank was made, and no kernel launched.

This measures only eager registration of the current library. It does not
measure future model modules, graph-private storage, collectives, final
workspaces, or model weights.

## Matched posture and provenance

The diagnostic reused the prior clean build without mutation:

```text
source commit   36584b08235052883159788d53cb62c9eba00941
image digest    sha256:2e401388dcc9c180401cb9997e3e8394c2db695c7bf4e3139ff8a9a517940719
binary SHA-256  b941c8434c3665d179d71fc9db6c103a14f12868a0ca9178014fb2a8c8c047ae
library SHA-256 79f15e86edf5ed14d7cd3402fab21cac47e191a24341a5b3daa0b344955f60b5
```

Each sample used a fresh container with all GPUs visible, network disabled,
private IPC, UID/GID 1000, read-only build mount, and only this environment
change:

```text
CUDA_MODULE_LOADING=EAGER
```

Raw evidence is outside Git at:

```text
/home/derek/glmaxx/evidence/20260803T165800Z-eager-memory-baseline-36584b0
```

The sorted external evidence hash list has SHA-256
`887a52a3a48c5eb5bde5d0db33dbc4b876c1aabfb0b1544c6e524697e1c1d09b`.
All stderr files are empty. Before execution the GPUs used 2/2/2/10 MiB with
no compute application; afterward they returned to the same memory posture
with no compute application.

## Capacity sensitivity

Using the unaccepted exact native-arena candidate rather than the minimum
record sum:

```text
eager minimum rank floor       101,365,645,312
immutable arena candidate       94,016,235,456
MTP3 cache candidate             4,330,317,824
residual for all other terms     3,019,092,032
old non-context fixed terms      2,550,136,832
provisional sensitivity margin     468,955,200
```

The fixed terms include the independent 1-GiB escrow. The 468,955,200-byte
margin is 2,097,152 bytes below the lazy-module sensitivity and remains only
a candidate arithmetic cross-check. Physical fit still requires accepted
native manifests, complete final modules/graphs/collectives/workspaces,
allocator reconciliation, all weight bytes, and fully written/checksummed KV
arenas on every rank.

