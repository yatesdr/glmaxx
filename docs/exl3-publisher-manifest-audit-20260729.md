# EXL3 publisher-manifest audit

Date: 2026-07-29

Evidence root:

```text
/home/derek/glmaxx/evidence/upstream-manifest-audit-9297b9f-r1
```

## Immutable inputs

| Input | Identity |
|---|---|
| repository | `brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw` |
| revision | `9297b9f1d53af5c67cffa01e30cc071a1ff7144b` |
| checkpoint | `/home/derek/models/GLM-5.2-EXL3-TR3-3.0bpw` |
| `MANIFEST.sha256` | `bfb6dc39f28da08c1cfc5b89603414046adf7003152d69e9ee350e11f7a1fa63` |
| manifest entries | 92 |
| weight shards | 81 |

## Result

A complete `sha256sum --check MANIFEST.sha256` read all 316 GB of checkpoint
payload. Ninety entries matched the publisher manifest and exactly two
non-model metadata entries did not:

| File | Manifest SHA-256 | Checkpoint SHA-256 | Exact-revision SHA-256 |
|---|---|---|---|
| `.gitattributes` | `34448b82c17d60fec9b65b1f093c115ddbaadc04beb1b0140b6bfed2e012a930` | `5bb36c320417db43af1dc6af8bd0fcc154bb7276eddaf96b12c395bdafed634d` | `5bb36c320417db43af1dc6af8bd0fcc154bb7276eddaf96b12c395bdafed634d` |
| `README.md` | `ed5aca8ce3dc5f8de626c87e488444343e43b1dcbdeb0e643dc72fea63ab06e8` | `e60e023082ee175a11f51e79e8dd88f5e4ed9975fc904e64cdeabbbcf8abe225` | `e60e023082ee175a11f51e79e8dd88f5e4ed9975fc904e64cdeabbbcf8abe225` |

Every model shard, the safetensors index, tokenizer/configuration file, and
other manifest entry matched. The two checkpoint metadata files were also
fetched independently from the immutable revision and matched the checkpoint
bytes exactly. Therefore the implementation permits exactly these two
publisher-manifest exception tuples. It does not permit a filename wildcard,
metadata class, or weight exception.

## Raw-record hashes

| Record | SHA-256 |
|---|---|
| `sha256-check.txt` | `2638e9b15b38d4da3095cbfca1f87b1fc108b87b4ec08afb1cc9ad15eef437b3` |
| `revision-metadata-sha256.txt` | `3cf44eff0d27229bdc4a2892432757221165e7ddaf5aad7ffbf163a6027cd720` |
| `checkpoint-metadata-sha256.txt` | `0a1de7ad2c8d6ad7e42e0df52b57f91d2258e407b3e70dcdd8c5f8d0a20ee53e` |
| `mismatches.txt` | `cd8aebe0bc758516ba0ec0d5cec51ec90f441c94823b0e8ddd840f738607ebbb` |
| `summary.txt` | `59cc58f570115da2f61d4c509f66e8a52a8a8cb65100b0d1a26c404fe2dc2899` |

This audit used no GPU and launched no CUDA work.
