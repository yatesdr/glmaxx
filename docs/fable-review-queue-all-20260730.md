# Fable review queue: all current handoffs

Date: 2026-08-03

Critical-path ordering for implementation unblock:
`docs/fable-critical-path-review-order-20260730.md`

Latest candidate included in the enumeration:
`1e2d0e2f10a363c2c3cdb79b73c419d49f5b10e2`

On a clean tracked tree, `review-proof-all` must verify 140 current handoffs
and skip the two historical umbrella handoffs
`docs/fable-phase-a-engine-handoff.md` and `docs/fable-review-handoff.md`.
Thirty-nine of 122 configured result artifacts are tracked and all 39 are
machine-accepted. The current operator-owned untracked review inbox adds four
explicitly withheld artifacts; it does not change the tracked acceptance
count.

The immediate source-to-first-batch review order is:

1. `docs/fable-safetensors-index-total-size-v1-r2-handoff.md`;
2. `docs/fable-exl3-mixed-k-source-and-kernel-v1-r2-handoff.md`;
3. `docs/fable-nf3-nvfp4-hybrid-source-and-kernel-v1-handoff.md`;
4. `docs/fable-target-layer-execution-v1-r2-handoff.md`;
5. `docs/fable-sm120-profiler-package-v1-handoff.md`; and
6. `docs/fable-current-tree-review-acceptance-v3-r2-handoff.md`.

The first three unblock real checkpoint CPU ingest. The target-layer r2 review
then opens the exact CPU operator-program and table proof. The profiler and
current-tree reviews bind the bytes allowed into the next SM120 qualification.
None conveys cn4 or CUDA authorization.

The resident-generation design review is also queued. It is not ahead of the
source-to-first-batch gates, but it must pass before implementing hot reload or
claiming zero weight traffic across tuning generations:
`docs/fable-resident-weight-runtime-generation-v1-handoff.md`.

The corrected quality/KLD contract is independently queued and may run in
parallel with source reviews. It must pass before evaluator implementation or
any GLMAXX quality claim:
`docs/fable-quality-acceptance-v1-r2-handoff.md`.
Its bounded source-recipe dependency is also queued at
`docs/fable-quality-corpus-sources-v1-r2-handoff.md`.

For every row, Fable should read the handoff itself and follow its exact
provenance, scope, result-path, and token rules. Review the pinned candidate
in a detached worktree; do not review moving `main`. A token in this table is
the requested token, not evidence of acceptance. If a handoff says it is
superseded, Fable should report that status without issuing its token. `—`
means the legacy handoff does not declare a machine-ingestable required
result path; follow the output instructions in that handoff.

| # | Handoff | Candidate | Required result | Requested token |
|---:|---|---|---|---|
| 1 | `docs/fable-active-prefix-record-binding-v1-handoff.md` | `92568f6045bf70a1d607435de318cebd6b4ef249` | `fable-active-prefix-record-binding-v1.md` | `active-prefix-record-binding-v1-accepted` |
| 2 | `docs/fable-active-sequence-page-table-v1-handoff.md` | `3404e070159be0d6932899111dda90865fdf2083` | — | design verdict only |
| 3 | `docs/fable-atomic-rank-publication-v1-handoff.md` | `aaeffeaf9899f32c353015965142bd0d25b91e3c` | `fable-atomic-rank-publication-v1.md` | `atomic-rank-publication-v1-accepted` |
| 4 | `docs/fable-backend-admission-rollback-fatal-v1-handoff.md` | `3ab31108f571c01ae4a83642c95e012d8b195123` | `fable-backend-admission-rollback-fatal-v1.md` | `backend-admission-rollback-fatal-v1-accepted` |
| 5 | `docs/fable-backend-event-cancellation-fatal-v1-handoff.md` | `0f0dd21204827f5893143ba93b7c71e9cc99d3c9` | `fable-backend-event-cancellation-fatal-v1.md` | `backend-event-cancellation-fatal-v1-accepted` |
| 6 | `docs/fable-backend-lossless-cancellation-v1-handoff.md` | `2ace56ccc9016c83422dbef1048371accd0430c8` | `fable-backend-lossless-cancellation-v1.md` | `backend-lossless-cancellation-v1-accepted` |
| 7 | `docs/fable-backend-runtime-readiness-v1-handoff.md` | `5ff3d48eef1a504bbbb0c65cfc9a0737dfcceac4` | `fable-backend-runtime-readiness-v1.md` | `backend-runtime-readiness-v1-accepted` |
| 8 | `docs/fable-cache-arena-budget-v2-handoff.md` | `c33648aa80ddfbcf3f40eaaec23d6d584a7fd543` | — | design verdict only |
| 9 | `docs/fable-checkpoint-load-transaction-v1-handoff.md` | `737603b4df40605ae47500c5ff9aec3a6b116293` | — | `checkpoint-load-transaction-v1-accepted` |
| 10 | `docs/fable-checkpoint-load-transaction-v1-r2-handoff.md` | `4bb0708b0a3931a6018ea0e5dfcb4bf07a5ae042` | — | `checkpoint-load-transaction-v1-r2-accepted` |
| 11 | `docs/fable-coordinator-api-backend-v1-handoff.md` | `5847a655c5751e3602b0abb7c322fc20cd975aed` | — | `coordinator-api-backend-v1-accepted` |
| 12 | `docs/fable-coordinator-api-backend-v2-handoff.md` | `8aaef8e50a69ed6fecdc01c6405dd6a2ff14ebc7` | — | `coordinator-api-backend-v2-accepted` |
| 13 | `docs/fable-current-tree-review-acceptance-v3-handoff.md` | `60311cfa6ec61c80fb0a1544dfe8121e3c3e0c7b` | `fable-current-tree-review-acceptance-v3.md` | `current-tree-review-acceptance-v3-design-accepted` |
| 14 | `docs/fable-direct-tier-io-v1-handoff.md` | `69895e040617a79dea78d7eaf1ced88234ccb193` | `fable-direct-tier-io-v1.md` | `direct-tier-io-v1-accepted` |
| 15 | `docs/fable-distributed-greedy-all-masked-v1-handoff.md` | `7867ed2e3839d74aad83f8b504bf5000247838b6` | `fable-distributed-greedy-all-masked-v1.md` | `distributed-greedy-all-masked-v1-accepted` |
| 16 | `docs/fable-distributed-sampling-abi-v1-handoff.md` | `7c718188b167615affabdb66f34939dcd6b22587` | — | `distributed-sampling-abi-v1-accepted` |
| 17 | `docs/fable-durable-catalog-extent-integrity-v1-handoff.md` | `de2d43a44474427d6f67fdb7fa300307d7b1caed` | `fable-durable-catalog-extent-integrity-v1.md` | `durable-catalog-extent-integrity-v1-accepted` |
| 18 | `docs/fable-durable-content-dedup-v1-handoff.md` | `b097703b0a6def10d3732ae70835881c93a954dd` | `fable-durable-content-dedup-v1.md` | `durable-content-dedup-v1-accepted` |
| 19 | `docs/fable-durable-journal-data-presence-v1-handoff.md` | `f72437917bec6df3ab8382575f5521ce491d356d` | `fable-durable-journal-data-presence-v1.md` | `durable-journal-data-presence-v1-accepted` |
| 20 | `docs/fable-durable-journal-transaction-sequence-v1-handoff.md` | `397c76c8e0b8e04e43c3f4ed19f1ac55ec730018` | `fable-durable-journal-transaction-sequence-v1.md` | `durable-journal-transaction-sequence-v1-accepted` |
| 21 | `docs/fable-durable-store-single-writer-v1-handoff.md` | `535a8d6764ff968a21cb5d668e1d895ef0e940fb` | `fable-durable-store-single-writer-v1.md` | `durable-store-single-writer-v1-accepted` |
| 22 | `docs/fable-durable-store-write-fail-stop-v1-handoff.md` | `a5019aafa7400f82928d944b0fb9a31ddae0605d` | `fable-durable-store-write-fail-stop-v1.md` | `durable-store-write-fail-stop-v1-accepted` |
| 23 | `docs/fable-exl3-source-projection-handoff.md` | `731c3bb02104edad0e154dcc63a26fe6bf224d7d` | — | `exl3-source-projection-v1-accepted` |
| 24 | `docs/fable-exl3-source-projection-v1-r2-handoff.md` | `0edfc8d796aeaeb969668005149bcb6286aa1e85` | `fable-exl3-source-projection-v1-r2.md` | `exl3-source-projection-v1-accepted` |
| 25 | `docs/fable-exl3-warp-decode-v2-handoff.md` | `c1ce8846013ecdd643493610eb134855779f3fac` | — | `exl3-warp-decode-v2-design-accepted` |
| 26 | `docs/fable-exl3-warp-decode-v2-r2-handoff.md` | `0edfc8d796aeaeb969668005149bcb6286aa1e85` | `fable-exl3-warp-decode-v2-r2.md` | `exl3-warp-decode-v2-design-accepted` |
| 27 | `docs/fable-fixed-page-transaction-v1-handoff.md` | `e1dd8d805801118750e0d93f3eb137fd5a493c0d` | `fable-fixed-page-transaction-v1.md` | `fixed-page-transaction-v1-accepted` |
| 28 | `docs/fable-generated-quality-corpus-v1-handoff.md` | `27fa48eab3d584920e8925abcbcc839be33a6485` | `fable-generated-quality-corpus-v1.md` | `generated-quality-corpus-v1-accepted` |
| 29 | `docs/fable-indexer-key-scale-v1-handoff.md` | `13f0c598c192f389ae664a22ffc2f81e58bd9f31` | `fable-indexer-key-scale-v1.md` | `indexer-key-scale-v1-accepted` |
| 30 | `docs/fable-journal-tail-corruption-v1-handoff.md` | `8612ec3a29421f707f0f231e3496a59bb81504b0` | `fable-journal-tail-corruption-v1.md` | `journal-tail-corruption-v1-accepted` |
| 31 | `docs/fable-kv-finite-reconstruction-v1-handoff.md` | `757d5cf44074a167a6434f708939719ef8550e1e` | `fable-kv-finite-reconstruction-v1.md` | `kv-finite-reconstruction-v1-accepted` |
| 32 | `docs/fable-manifest-abi-v022-handoff.md` | `22d03fcce921483bbf71da5a51e80131326217b7` | — | `manifest-abi-v0.2.2-accepted` |
| 33 | `docs/fable-manifest-abi-v022-r2-handoff.md` | `0edfc8d796aeaeb969668005149bcb6286aa1e85` | `fable-manifest-abi-v022-r2.md` | `manifest-abi-v0.2.2-accepted` |
| 34 | `docs/fable-mtp-layer-execution-v1-handoff.md` | `fd80e16d88434fdf7bf55778977044c64dd1a366` | `fable-mtp-layer-execution-v1.md` | `mtp-layer-execution-v1-accepted` |
| 35 | `docs/fable-nonblocking-http-transport-v1-handoff.md` | `3608a030530d2157fedc25a1432cd769ec8e9f98` | — | `nonblocking-http-transport-v1-accepted` |
| 36 | `docs/fable-normative-startup-order-v1-handoff.md` | `7420657a8528ef2ed780974bb0b8a699db9cfb0f` | `fable-normative-startup-order-v1.md` | `normative-startup-order-v1-accepted` |
| 37 | `docs/fable-nvfp4-metadata-canonicality-v1-handoff.md` | `9d30aa17cc60de9215b598e4056c880446702f94` | `fable-nvfp4-metadata-canonicality-v1.md` | `nvfp4-metadata-canonicality-v1-accepted` |
| 38 | `docs/fable-nvfp4-streaming-canonicality-v2-handoff.md` | `9262007eaf675d2bc1754c0f17a3ae8a871abb18` | `fable-nvfp4-streaming-canonicality-v2.md` | `nvfp4-streaming-canonicality-v2-accepted` |
| 39 | `docs/fable-offline-foundation-handoff.md` | `fc70f6626acae58746c9672482330ae151917b8d` | — | design verdict only |
| 40 | `docs/fable-online-prefix-publication-v1-handoff.md` | `d0a09d7c62f1943112eaa703a9ef3f6b25e9ebc9` | — | `online-prefix-publication-v1-accepted` |
| 41 | `docs/fable-page-reuse-quarantine-v1-handoff.md` | `832bf9784ae67b2db4891bb17dcb8fc2647cf53a` | `fable-page-reuse-quarantine-v1.md` | `page-reuse-quarantine-v1-accepted` |
| 42 | `docs/fable-page-table-delta-v1-handoff.md` | `a1d4cb48331b229a683ffa90ba41a609d74ad261` | `fable-page-table-delta-v1.md` | `page-table-delta-v1-accepted` |
| 43 | `docs/fable-pending-admission-rollback-v1-handoff.md` | `bfbe7f46cbd9db52aa766950aec1432c7677778d` | `fable-pending-admission-rollback-v1.md` | `pending-admission-rollback-v1-accepted` |
| 44 | `docs/fable-plain-padding-streaming-v1-handoff.md` | `a3f44531c7494cd9c0aee8bd58dd7c43bb657fb6` | `fable-plain-padding-streaming-v1.md` | `plain-padding-streaming-v1-accepted` |
| 45 | `docs/fable-prefill-captured-shape-v1-handoff.md` | `9bdb2084619f0ede4425da3c626993b96fc3e6f8` | `fable-prefill-captured-shape-v1.md` | `prefill-captured-shape-v1-accepted` |
| 46 | `docs/fable-prefill-graph-profile-abi-v2-handoff.md` | `9b0465284a1c0845772551902bb3e21c26025d51` | `fable-prefill-graph-profile-abi-v2.md` | `prefill-graph-profile-abi-v2-design-accepted` |
| 47 | `docs/fable-prefix-generation-integrity-v1-handoff.md` | `2e3aa222e0808c27793798dab6890dbdb7614ed3` | `fable-prefix-generation-integrity-v1.md` | `prefix-generation-integrity-v1-accepted` |
| 48 | `docs/fable-prefix-release-atomicity-v1-handoff.md` | `14b97a2de700973ef3132aeb446659e1c3d6edf6` | `fable-prefix-release-atomicity-v1.md` | `prefix-release-atomicity-v1-accepted` |
| 49 | `docs/fable-prefix-residency-coherence-v1-handoff.md` | `72e60716cf58632dd9aba5ead41ba0d128f59395` | `fable-prefix-residency-coherence-v1.md` | `prefix-residency-coherence-v1-accepted` |
| 50 | `docs/fable-production-rank-manifest-validation-v1-handoff.md` | `46bff28aaf950ea15fdfc69ac074412cbd46c9c4` | `fable-production-rank-manifest-validation-v1.md` | `production-rank-manifest-validation-v1-accepted` |
| 51 | `docs/fable-production-rank-manifest-validation-v2-handoff.md` | `4bf7bb5e817e01cc299058b56a488b35011fd79d` | `fable-production-rank-manifest-validation-v2.md` | `production-rank-manifest-validation-v2-accepted` |
| 52 | `docs/fable-quality-acceptance-v1-handoff.md` | `70222ab17ea5c10bdb5a68e98a1a5839a040eec9` | — | `quality-acceptance-v1-accepted` |
| 53 | `docs/fable-quality-corpus-sources-v1-handoff.md` | `83fb3747ae5d5ae996edb4784f775836e7c1a3e6` | `fable-quality-corpus-sources-v1.md` | `quality-corpus-sources-v1-accepted` |
| 54 | `docs/fable-rank-mirror-step-transaction-v1-handoff.md` | `414b8464a298eb749f6bb22e9f56987cc19634e3` | `fable-rank-mirror-step-transaction-v1.md` | `rank-mirror-step-transaction-v1-accepted` |
| 55 | `docs/fable-rank-residency-content-identity-v1-handoff.md` | `eceee043cedab38b30f4f64cd5871eede0a254e5` | `fable-rank-residency-content-identity-v1.md` | `rank-residency-content-identity-v1-accepted` |
| 56 | `docs/fable-residency-admission-atomicity-v1-handoff.md` | `c84da2a4686c37227de5a0dd4694409fdf42f25b` | `fable-residency-admission-atomicity-v1.md` | `residency-admission-atomicity-v1-accepted` |
| 57 | `docs/fable-restore-identity-v1-handoff.md` | `dc16273b019cf3a3dd8eb810cf9caeb26c99bced` | `fable-restore-identity-v1.md` | `restore-identity-v1-accepted` |
| 58 | `docs/fable-restore-operation-quota-v1-handoff.md` | `95683d8d5ea1c31f1f9299ac5956dea99ef3ca63` | `fable-restore-operation-quota-v1.md` | `restore-operation-quota-v1-accepted` |
| 59 | `docs/fable-retained-http-request-ownership-v1-handoff.md` | `a7b1cc9a6cbae1d5abce75c672693759ac584794` | `fable-retained-http-request-ownership-v1.md` | `retained-http-request-ownership-v1-accepted` |
| 60 | `docs/fable-retained-http-startup-cleanup-v1-handoff.md` | `20c773c94179b2ab0913ed69eaf82a301d6b27db` | `fable-retained-http-startup-cleanup-v1.md` | `retained-http-startup-cleanup-v1-accepted` |
| 61 | `docs/fable-scheduler-batch-atomicity-v1-handoff.md` | `2f7d0ce30392d1fe5c3256058e4d8604100791f2` | `fable-scheduler-batch-atomicity-v1.md` | `scheduler-batch-atomicity-v1-accepted` |
| 62 | `docs/fable-selected-step-failure-finalization-v1-handoff.md` | `2ff0ac124be63a8a8318d664d167f34dde32ed3c` | `fable-selected-step-failure-finalization-v1.md` | `selected-step-failure-finalization-v1-accepted` |
| 63 | `docs/fable-sequence-removal-atomicity-v1-handoff.md` | `876e4ca59be4c7a8243288c57cf79ef3cbebc5d4` | `fable-sequence-removal-atomicity-v1.md` | `sequence-removal-atomicity-v1-accepted` |
| 64 | `docs/fable-serving-active-page-transaction-v1-handoff.md` | `326158a25f6ca0c68e1b543195984c5537542df4` | `fable-serving-active-page-transaction-v1.md` | `serving-active-page-transaction-v1-accepted` |
| 65 | `docs/fable-serving-observability-v1-handoff.md` | `9607aa0e3027dc998bc9489c7abe29320c7b7972` | — | `serving-observability-v1-accepted` |
| 66 | `docs/fable-serving-page-transaction-v1-handoff.md` | `e7bc4778119d43da7e1c76bfc584e5993d1fbb73` | — | design verdict only |
| 67 | `docs/fable-sm120-rank-executor-v1-handoff.md` | `b64cb6dba506b7b5f6f2cac48c5f17b3b920d3bc` | `fable-sm120-rank-executor-v1.md` | `sm120-rank-executor-v1-accepted` |
| 68 | `docs/fable-step-execution-io-v1-handoff.md` | `a5ef0764a4c36c01553006dd4041fb23233bd559` | — | design verdict only |
| 69 | `docs/fable-step-input-page-delta-binding-v1-handoff.md` | `b351e44c2cddf94a12fb8d10f8632119670ca2a9` | `fable-step-input-page-delta-binding-v1.md` | `step-input-page-delta-binding-v1-accepted` |
| 70 | `docs/fable-streaming-write-single-pass-v1-handoff.md` | `5ff1f8541f0fdbb14b2923694d8cc4d444470b55` | `fable-streaming-write-single-pass-v1.md` | `streaming-write-single-pass-v1-accepted` |
| 71 | `docs/fable-sustained-serving-load-fault-v1-handoff.md` | `1dbab21c636e495947f384751dafd219a995ad18` | `fable-sustained-serving-load-fault-v1.md` | `sustained-serving-load-fault-v1-accepted` |
| 72 | `docs/fable-target-layer-execution-v1-handoff.md` | `83f5005a7e6dd3f45422df6cb091c4e743727bbd` | `fable-target-layer-execution-v1.md` | `target-layer-execution-v1-accepted` |
| 73 | `docs/fable-tenant-resource-quotas-v1-handoff.md` | `7e810c43a8856e09d48314dfef3959ded93c5f8f` | — | `tenant-resource-quotas-v1-accepted` |
| 74 | `docs/fable-terminal-cleanup-transaction-v1-handoff.md` | `6535248bb217b20d56ec0d6670c8fb6f33791205` | `fable-terminal-cleanup-transaction-v1.md` | `terminal-cleanup-transaction-v1-accepted` |
| 75 | `docs/fable-torn-journal-resume-v1-handoff.md` | `8fb3adf9535683b0de9b54fe2743cb5651b9bdc2` | `fable-torn-journal-resume-v1.md` | `torn-journal-resume-v1-accepted` |
| 76 | `docs/fable-tp4-rank-startup-handshake-v1-handoff.md` | `1eb8e1c2f6c98a2d20b8e4f168b8e88aadeb97ac` | `fable-tp4-rank-startup-handshake-v1.md` | `tp4-rank-startup-handshake-v1-accepted` |
| 77 | `docs/fable-tp4-step-operation-quota-v1-handoff.md` | `da46a30a5df430e35d4a9d23aa6a449923494660` | `fable-tp4-step-operation-quota-v1.md` | `tp4-step-operation-quota-v1-accepted` |
| 78 | `docs/fable-small-checkpoint-runner-v1-handoff.md` | `2b3318176d34eded55cc97e49998423ad4e902ce` | `docs/reviews/fable-small-checkpoint-runner-v1.md` | superseded; do not issue token |
| 79 | `docs/fable-checkpoint-load-cpu-core-v1-handoff.md` | `d29ce96a0d6037e045c359fd1116187ca0722c42` | `docs/reviews/fable-checkpoint-load-cpu-core-v1.md` | `checkpoint-load-cpu-core-v1-accepted` |
| 80 | `docs/fable-native-rank-load-plan-v1-handoff.md` | `7681210af6c93d7a2cb644a80d8aa001e8e8cc02` | `docs/reviews/fable-native-rank-load-plan-v1.md` | `native-rank-load-plan-v1-accepted` |
| 81 | `docs/fable-rank-set-load-coordinator-v1-handoff.md` | `51e8dc185d328f6fb9cda84ec5e75de14e756776` | `docs/reviews/fable-rank-set-load-coordinator-v1.md` | `rank-set-load-coordinator-v1-accepted` |
| 82 | `docs/fable-cuda-checkpoint-arena-cpu-v1-handoff.md` | `c870454025ea9a401646155c010e1032b23659d8` | `docs/reviews/fable-cuda-checkpoint-arena-cpu-v1.md` | `cuda-checkpoint-arena-cpu-v1-accepted` |
| 83 | `docs/fable-rank-local-checkpoint-loader-v1-handoff.md` | `9c345421557a0a4e290831c61afcf65cf3f53a10` | `docs/reviews/fable-rank-local-checkpoint-loader-v1.md` | `rank-local-checkpoint-loader-v1-accepted` |
| 84 | `docs/fable-tp4-checkpoint-load-protocol-v1-handoff.md` | `d64753549881f7ecb5a3920bff888d81ee3345a0` | `docs/reviews/fable-tp4-checkpoint-load-protocol-v1.md` | `tp4-checkpoint-load-protocol-v1-accepted` |
| 85 | `docs/fable-native-checkpoint-rank-adapter-v1-handoff.md` | `b62325a47eaee6b78bd70ec29e3ea29cea48533e` | `docs/reviews/fable-native-checkpoint-rank-adapter-v1.md` | `native-checkpoint-rank-adapter-v1-accepted` |
| 86 | `docs/fable-native-checkpoint-startup-composition-v1-handoff.md` | `b55c8a9169c72a311c59d30e5389618eef3f0d7b` | `docs/reviews/fable-native-checkpoint-startup-composition-v1.md` | `native-checkpoint-startup-composition-v1-accepted` |
| 87 | `docs/fable-native-checkpoint-load-smoke-v1-handoff.md` | `1770563713722685db26b0d3378f32e4ecf92519` | `docs/reviews/fable-native-checkpoint-load-smoke-v1.md` | `native-checkpoint-load-smoke-v1-accepted` |
| 88 | `docs/fable-resident-tensor-device-binding-v1-handoff.md` | `a49210fe384012066d80087f61668d5d8a8e2a78` | `docs/reviews/fable-resident-tensor-device-binding-v1.md` | `resident-tensor-device-binding-v1-accepted` |
| 89 | `docs/fable-target-program-projection-discriminator-v1-handoff.md` | `39fbee5bf220467104535d86c00b49effe96c3a8` | `docs/reviews/fable-target-program-projection-discriminator-v1.md` | superseded; do not issue token |
| 90 | `docs/fable-sm120-rank-executor-v1-r2-handoff.md` | `a0f2bee3edd1754aebefe1643eecd0a63cd4d4b7` | `docs/reviews/fable-sm120-rank-executor-v1-r2.md` | `sm120-rank-executor-v1-accepted` |
| 91 | `docs/fable-step-execution-abi-v3-handoff.md` | `bab7866b6bd494d3e70ba28463043555f5b583c8` | `docs/reviews/fable-step-execution-abi-v3.md` | `step-execution-abi-v3-design-accepted` |
| 92 | `docs/fable-exl3-warp-staging-cpu-v2-handoff.md` | `c1ab9d2214f592e02de2cf3e7f2dfb257930b347` | `docs/reviews/fable-exl3-warp-staging-cpu-v2.md` | `exl3-warp-staging-cpu-v2-accepted` |
| 93 | `docs/fable-direct-tier-extent-cpu-v1-handoff.md` | `8c27c1e6082f35cc225a8ed76255bd2724c47c6c` | `docs/reviews/fable-direct-tier-extent-cpu-v1.md` | `direct-tier-extent-cpu-v1-accepted` |
| 94 | `docs/fable-direct-tier-state-cpu-v1-handoff.md` | `ccd636967bec031c8a8b0349a18b39113c0a6ae6` | `docs/reviews/fable-direct-tier-state-cpu-v1.md` | `direct-tier-state-cpu-v1-accepted` |
| 95 | `docs/fable-direct-tier-durable-format-v1-handoff.md` | `96be26e8a1d43cac047cd57a38bf3d13f6dbb756` | `docs/reviews/fable-direct-tier-durable-format-v1.md` | `direct-tier-durable-format-v1-design-accepted` |
| 96 | `docs/fable-hbm-dram-transfer-v1-handoff.md` | `839f377473e08994269bcac68881f3e7afa14790` | `docs/reviews/fable-hbm-dram-transfer-v1.md` | `hbm-dram-transfer-v1-design-accepted` |
| 97 | `docs/fable-nvfp4-fused-routed-moe-v1-handoff.md` | `803ae518424aef00d98e09e73b940f6b2c9832ca` | `docs/reviews/fable-nvfp4-fused-routed-moe-v1.md` | superseded; do not issue token |
| 98 | `docs/fable-online-prefix-publication-v1-r2-handoff.md` | `a9b40f1b1440797a05543d5e65e61927fd141b97` | `docs/reviews/fable-online-prefix-publication-v1-r2.md` | `online-prefix-publication-v1-accepted` |
| 99 | `docs/fable-rank-residency-content-identity-v1-r2-handoff.md` | `386ea9a61bae10836a97efec24176118ee8e7632` | `docs/reviews/fable-rank-residency-content-identity-v1-r2.md` | `rank-residency-content-identity-v1-accepted` |
| 100 | `docs/fable-restore-operation-quota-v1-r2-handoff.md` | `12c0c49c0ab966101eaf2797a3a23555ec069b2f` | `docs/reviews/fable-restore-operation-quota-v1-r2.md` | `restore-operation-quota-v1-accepted` |
| 101 | `docs/fable-kv-reconstruction-rounding-v2-handoff.md` | `1212fe9bf39f690401df8e49dcaca44708502a20` | `docs/reviews/fable-kv-reconstruction-rounding-v2.md` | `kv-reconstruction-rounding-v2-design-accepted` |
| 102 | `docs/fable-tp4-worker-admission-state-v1-handoff.md` | `46f251e8af7d0b75593c7ad66c00ae41dcd3f7a8` | `docs/reviews/fable-tp4-worker-admission-state-v1.md` | `tp4-worker-admission-state-v1-accepted` |
| 103 | `docs/fable-nonblocking-http-transport-v1-r2-handoff.md` | `b7a2ac4bd45b1cb7a15c69c33d7d2248da826ad5` | `docs/reviews/fable-nonblocking-http-transport-v1-r2.md` | `nonblocking-http-transport-v1-r2-design-accepted` |
| 104 | `docs/fable-coordinator-api-backend-v3-handoff.md` | `10a068ba55cc0e8dbe39161f925a0dcf0a17d8ef` | `docs/reviews/fable-coordinator-api-backend-v3.md` | `coordinator-api-backend-v3-accepted` |
| 105 | `docs/fable-streaming-write-single-pass-v1-r2-handoff.md` | `f39f23495b80dd7527c379788d39f58987ed2b52` | `docs/reviews/fable-streaming-write-single-pass-v1-r2.md` | `streaming-write-single-pass-v1-accepted` |
| 106 | `docs/fable-fixed-page-transaction-v1-r2-handoff.md` | `b59114734e1fb18761725444e27fbe9c64b6ad43` | `docs/reviews/fable-fixed-page-transaction-v1-r2.md` | `fixed-page-transaction-v1-r2-design-accepted` |
| 107 | `docs/fable-checkpoint-load-transaction-v1-r3-handoff.md` | `fc96d90836e32d7c582a1bddbf1521a28638ccfa` | `docs/reviews/fable-checkpoint-load-transaction-v1-r3.md` | `checkpoint-load-transaction-v1-r3-accepted` |
| 108 | `docs/fable-cache-arena-budget-v2-r2-handoff.md` | `04e4c3ac60a50ba6bb3a9767bbe43c3d68cec614` | `docs/reviews/fable-cache-arena-budget-v2-r2.md` | `cache-arena-budget-v2-r2-cpu-accepted` |
| 109 | `docs/fable-direct-tier-scheduler-cpu-v1-handoff.md` | `6cdbeae417e053d08751c8102304064bf86c360e` | `docs/reviews/fable-direct-tier-scheduler-cpu-v1.md` | superseded; do not issue token |
| 110 | `docs/fable-direct-tier-scheduler-cpu-v1-r2-handoff.md` | `e188fc7fcd31c7ca35a48750ff2933267dd40111` | `docs/reviews/fable-direct-tier-scheduler-cpu-v1-r2.md` | superseded; do not issue token |
| 111 | `docs/fable-direct-tier-scheduler-cpu-v1-r3-handoff.md` | `b602a9c26f1821f70b2872b158a2201155f71ef1` | `docs/reviews/fable-direct-tier-scheduler-cpu-v1-r3.md` | `direct-tier-scheduler-cpu-v1-r3-accepted` |
| 112 | `docs/fable-direct-tier-linux-probe-v1-handoff.md` | `a3771e31dd73132db1caa306c097408e05388988` | `docs/reviews/fable-direct-tier-linux-probe-v1.md` | `direct-tier-linux-probe-v1-design-accepted` |
| 113 | `docs/fable-cn4-environment-capture-v1-handoff.md` | `d961d96da3f4b99052272afcd4abf28dbe6f9854` | `docs/reviews/fable-cn4-environment-capture-v1.md` | `cn4-environment-capture-v1-design-accepted` |
| 114 | `docs/fable-matched-runtime-control-v1-handoff.md` | `660a0707bb4b0a67f3c3983b4cef1dc18a38b6b1` | `docs/reviews/fable-matched-runtime-control-v1.md` | `matched-runtime-control-v1-design-accepted` |
| 115 | `docs/fable-nvfp4-fused-routed-moe-v1-r2-handoff.md` | `2afb205f7cfe5c90cdeac1262996b9fb9df0f726` | `docs/reviews/fable-nvfp4-fused-routed-moe-v1-r2.md` | superseded; do not issue token |
| 116 | `docs/fable-nvfp4-fused-routed-moe-v1-r3-handoff.md` | `bcc8ebf0b951516acb63ebf2baea1825018bbed8` | `docs/reviews/fable-nvfp4-fused-routed-moe-v1-r3.md` | `nvfp4-fused-routed-moe-v1-r3-design-accepted` |
| 117 | `docs/fable-nvfp4-laboratory-manifest-v1-handoff.md` | `0e084967f6750253c584ca3b0221dbc6de382a30` | `docs/reviews/fable-nvfp4-laboratory-manifest-v1.md` | `nvfp4-laboratory-manifest-v1-design-accepted` |
| 118 | `docs/fable-small-checkpoint-runner-v1-r2-handoff.md` | `16922e4c699b8145eb8d43455e5626b13679ea60` | `docs/reviews/fable-small-checkpoint-runner-v1-r2.md` | `small-checkpoint-runner-v1-r2-design-accepted` |
| 119 | `docs/fable-tp4-layer6-replay-v1-handoff.md` | `4a1e3766d15440f63873b3c50203080121be0d7e` | `docs/reviews/fable-tp4-layer6-replay-v1.md` | `tp4-layer6-replay-v1-design-accepted` |
| 120 | `docs/fable-hybrid-serving-manifest-v1-handoff.md` | `67db1a33de774762f724ca8157fccb1a0d689e4d` | `docs/reviews/fable-hybrid-serving-manifest-v1.md` | `hybrid-serving-manifest-v1-design-accepted` |
| 121 | `docs/fable-direct-tier-checksum-authority-cpu-v1-handoff.md` | `7267b505fd4b83c9b421e5050277bf806a1e4867` | `docs/reviews/fable-direct-tier-checksum-authority-cpu-v1.md` | `direct-tier-checksum-authority-cpu-v1-accepted` |
| 122 | `docs/fable-direct-tier-checksum-workers-cpu-v1-handoff.md` | `59f70da4dbeca8a5d542f3e5947002d3ee975bdb` | `docs/reviews/fable-direct-tier-checksum-workers-cpu-v1.md` | `direct-tier-checksum-workers-cpu-v1-accepted` |
| 123 | `docs/fable-current-tree-review-acceptance-v3-r2-handoff.md` | `1efdf101f42439587b38c147f8a9ff5ee8b59ba5` | `fable-current-tree-review-acceptance-v3-r2.md` | `current-tree-review-acceptance-v3-r2-design-accepted` |
| 124 | `docs/fable-sm120-profiler-package-v1-handoff.md` | `fdbd91647a3ea23031ebd562e3d57676d7eb5d9a` | `fable-sm120-profiler-package-v1.md` | `sm120-profiler-package-v1-accepted` |
| 125 | `docs/fable-safetensors-index-total-size-v1-handoff.md` | `bb8b7d6b9529b69bfaa3d9b981df7412e39bb30b` | `fable-safetensors-index-total-size-v1.md` | superseded; do not issue token |
| 126 | `docs/fable-exl3-mixed-k-source-and-kernel-v1-handoff.md` | `849c1d12bf42d92aecffe9003530a2a13dcc3dfe` | `fable-exl3-mixed-k-source-and-kernel-v1.md` | superseded; do not issue token |
| 127 | `docs/fable-nf3-nvfp4-hybrid-source-and-kernel-v1-handoff.md` | `d3a5acd91422845a4665898405f05466763b8525` | `fable-nf3-nvfp4-hybrid-source-and-kernel-v1.md` | `nf3-nvfp4-hybrid-source-and-kernel-v1-design-accepted` |
| 128 | `docs/fable-resident-weight-runtime-generation-v1-handoff.md` | `9710c0db7245592a17084b65efe041010612bcfa` | `fable-resident-weight-runtime-generation-v1.md` | `resident-weight-runtime-generation-v1-design-accepted` |
| 129 | `docs/fable-target-layer-execution-v1-r2-handoff.md` | `d4817ff9ff7eec09c74e98a99db5c27690286013` | `fable-target-layer-execution-v1-r2.md` | `target-layer-execution-v1-accepted` |
| 130 | `docs/fable-quality-acceptance-v1-r2-handoff.md` | `eb62b3d138880e7bfcacec74f975de5a017cd977` | `fable-quality-acceptance-v1-r2.md` | `quality-acceptance-v1-accepted` |
| 131 | `docs/fable-quality-corpus-sources-v1-r2-handoff.md` | `a2fc47afb8557fb0b8a3396865fb951064380dad` | `fable-quality-corpus-sources-v1-r2.md` | `quality-corpus-sources-v1-accepted` |
| 132 | `docs/fable-cn4-tp4-eager-module-baseline-20260803-handoff.md` | `f19e66e082ee8a2ace2b59db04c96e58295c0fb9` | `fable-cn4-tp4-eager-module-baseline-20260803.md` | `cn4-tp4-eager-module-baseline-20260803-diagnostic-accepted` |
| 133 | `docs/fable-cn4-tp4-memory-baseline-20260803-handoff.md` | `28993cf858357ee5b697e8fca1f94d2136c6e233` | `fable-cn4-tp4-memory-baseline-20260803.md` | `cn4-tp4-memory-baseline-20260803-diagnostic-accepted` |
| 134 | `docs/fable-fc1-direct-control-oracle-r1-handoff.md` | `da65e63c8ebbe303335ca2636a3b56d7f1dfe028` | `fable-fc1-direct-control-oracle-r1.md` | `fc1-direct-control-oracle-r1-design-accepted` |
| 135 | `docs/fable-fc2-grouped-control-scratch-r1-handoff.md` | `da65e63c8ebbe303335ca2636a3b56d7f1dfe028` | `fable-fc2-grouped-control-scratch-r1.md` | superseded; do not issue token |
| 136 | `docs/fable-hybrid-mtp3-capacity-ledger-v1-handoff.md` | `94b09e7a2c4281116f38eed10b4ce97e35ebf833` | `fable-hybrid-mtp3-capacity-ledger-v1.md` | `hybrid-mtp3-capacity-ledger-v1-design-accepted` |
| 137 | `docs/fable-nf3-nvfp4-native-rank-manifest-v1-handoff.md` | `b1584a989f5878c1b433ea54ffb0dc2925b03f9e` | `fable-nf3-nvfp4-native-rank-manifest-v1.md` | `nf3-nvfp4-native-rank-manifest-v1-design-accepted` |
| 138 | `docs/fable-exl3-mixed-k-source-and-kernel-v1-r2-handoff.md` | `23e6e26c172e370b63608a07aa2f781a24faef24` | `fable-exl3-mixed-k-source-and-kernel-v1-r2.md` | `exl3-mixed-k-source-and-kernel-v1-r2-design-accepted` |
| 139 | `docs/fable-fc2-grouped-control-scratch-r2-handoff.md` | `419c2b0832723f5ffaeecbbc39c9ad6fd8652be7` | `fable-fc2-grouped-control-scratch-r2.md` | `fc2-grouped-control-scratch-r2-design-accepted` |
| 140 | `docs/fable-safetensors-index-total-size-v1-r2-handoff.md` | `1e2d0e2f10a363c2c3cdb79b73c419d49f5b10e2` | `fable-safetensors-index-total-size-v1-r2.md` | `safetensors-index-total-size-v1-r2-design-accepted` |

## Verification command

Before dispatching reviews, regenerate the machine proof:

```text
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof-all . /tmp/glmaxx-review-provenance.json
```

The expected queue count for this document is 140 current handoffs and two
explicitly skipped historical umbrella handoffs.
