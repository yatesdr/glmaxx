# Pinned tokenizer and streaming text contract

Date: 2026-07-29

Status: implemented CPU candidate; independent review remains required by
`spec/engine-v0.md` OPEN item 12

The serving path accepts only the tokenizer bundle from the pinned
`brandonmusic/GLM-5.2-EXL3-TR3-3.0bpw` revision
`9297b9f1d53af5c67cffa01e30cc071a1ff7144b`. Startup verifies regular,
non-symlink files, exact byte lengths, and these SHA-256 identities before
constructing the tokenizer:

| File | Bytes | SHA-256 |
|---|---:|---|
| `tokenizer.json` | 20,217,442 | `19e773648cb4e65de8660ea6365e10acca112d42a854923df93db4a6f333a82d` |
| `tokenizer_config.json` | 761 | `98b1271574f41abf89427ae2dda030d94dc9478f0edc5a8bd240db213c6fd5fc` |
| `generation_config.json` | 194 | `ac76b43d8683d3b930126870fc8be73d8679308fe752fa1f381096d8354f6a55` |
| `chat_template.jinja` | 5,076 | `172dc74a35e1752df75ecfb2b2cf9326d2852bb1379868ebeec9571654489679` |

`glm-tokenizer` loads the audited ByteLevel BPE through the pure-Rust
`tokenizers` core with hub, progress-bar, Oniguruma, and training-oriented
default features disabled. No runtime network access or Python process is
used.

## Vocabulary boundary

The model head has 154,880 rows. The tokenizer maps IDs `0..154855`, while
IDs `154856..154879` are padding rows with no text representation. Those 24
logits must be masked before greedy or probabilistic selection. An unmapped
ID reaching detokenization is a hard error.

The complete ID-to-output-byte table, including special and invalid entries,
has SHA-256:

```text
31b20a4136e6f2854e40bdc34396cfcb6e893259c335fe6d1bdbfef48ea5fa1a
```

The EOS set is exactly `154820`, `154827`, and `154829`.

## Chat-template specialization

The generic Jinja interpreter is not in the serving hot path. A fixed Rust
renderer implements the audited GLM-5.2 template:

- `[gMASK]<sop>` and reasoning-effort preamble;
- system, user, assistant, and observation roles;
- thinking enabled/disabled and history-clearing behavior;
- ordered tool schemas and ordered tool-call arguments;
- generation-prompt suffix.

Tool JSON uses an order-preserving deserializer because the pinned template
iterates object mappings. Converting through the normal sorted
`serde_json::Value` representation would change the token sequence.
Tool-call arguments may arrive as an object or as an OpenAI-style JSON
string; both normalize to the same ordered object before rendering.

## Incremental output

ByteLevel tokens may divide one UTF-8 scalar across multiple token IDs.
`IncrementalDecoder` retains incomplete byte suffixes, emits only valid UTF-8,
skips special-token text, and uses replacement semantics only for a genuinely
invalid or final incomplete sequence.

Stop strings are matched across token and UTF-8 boundaries. The decoder
withholds only the longest suffix that could still become a configured stop,
so no stop prefix leaks to a streaming client and buffering remains bounded
by the maximum 256-byte stop plus an incomplete UTF-8 scalar. EOS releases an
incomplete stop prefix because no later token can complete it.

## Reproduction

The proof command requires the external four-file bundle and writes no
tokenizer bytes to Git:

```bash
cargo run --release --offline -p glm-cli -- \
  tokenizer-proof /path/to/pinned/checkpoint /tmp/tokenizer-proof.json
cmp fixtures/tokenizer-contract-proof-v1.json /tmp/tokenizer-proof.json
```

The checked fixture covers ASCII, Unicode, code, simple chat, chat-history
token IDs, complete token-output-table identity, incremental UTF-8, and a
cross-token stop. Its reference IDs were independently generated with the
Python binding of tokenizer core 0.22.2; the Rust runtime uses 0.23.1.

This closes implementation work for the CPU candidate, not the serving gate.
The production backend still has to mask padding logits before every
distributed sampling route and connect committed token IDs to this decoder.
