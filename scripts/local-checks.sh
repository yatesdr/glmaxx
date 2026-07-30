#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

proof_dir="$(mktemp -d "${TMPDIR:-/tmp}/glmaxx-local-proof.XXXXXX")"
trap 'rm -rf "${proof_dir}"' EXIT

cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
GLMAXX_KERNEL_LIB_DIR="${proof_dir}" \
  cargo check --offline -p glm-cli --features cuda-ffi
GLMAXX_KERNEL_LIB_DIR="${proof_dir}" \
  cargo clippy --offline -p glm-cli --features cuda-ffi -- -D warnings
cargo run --release --offline -p glm-cli --bin glmaxx -- cpu-proof
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  exl3-warp-proof "${proof_dir}/exl3-warp-staging-proof-v2.json"
cmp fixtures/exl3-warp-staging-proof-v2.json \
  "${proof_dir}/exl3-warp-staging-proof-v2.json"
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  matrix-proof "${proof_dir}/matrix-proof.json"
cmp fixtures/sm120-fc1-matrix-proof-v1.json \
  "${proof_dir}/matrix-proof.json"
cargo run --offline -p glm-cli --bin glmaxx -- manifest "${proof_dir}/manifest.json"
cmp manifests/glm52-operation-v1.json "${proof_dir}/manifest.json"
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  pack-actual "${proof_dir}/rank0.g5n"
cargo run --offline -p glm-cli --bin glmaxx -- \
  inspect "${proof_dir}/rank0.g5n"
cargo run --offline -p glm-cli --bin glmaxx -- budget
cargo run --offline -p glm-cli --bin glmaxx -- abi-check
cargo run --offline -p glm-cli --bin glmaxx -- \
  review-proof-all . "${proof_dir}/review-provenance.json"
cargo run --offline -p glm-cli --bin glmaxx -- \
  engine-proof "${proof_dir}/engine-proof.json"
cmp fixtures/engine-contract-proof-v1.json \
  "${proof_dir}/engine-proof.json"
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  serving-proof "${proof_dir}/serving"
cmp fixtures/cpu-serving-proof-v1.json \
  "${proof_dir}/serving/serving-proof.json"
cargo run --release --offline -p glm-cli --bin glmaxx -- \
  cache-lifecycle-proof "${proof_dir}/cache-lifecycle"
cmp fixtures/cache-lifecycle-proof-v1.json \
  "${proof_dir}/cache-lifecycle/cache-lifecycle-proof.json"
if [[ -n "${GLMAXX_TOKENIZER_DIR:-}" ]]; then
  cargo run --release --offline -p glm-cli --bin glmaxx -- \
    tokenizer-proof "${GLMAXX_TOKENIZER_DIR}" \
    "${proof_dir}/tokenizer-proof.json"
  cmp fixtures/tokenizer-contract-proof-v1.json \
    "${proof_dir}/tokenizer-proof.json"
else
  echo "Pinned tokenizer proof skipped: GLMAXX_TOKENIZER_DIR is not set"
fi

clang++ -std=c++17 -fsyntax-only -x c++ kernels/include/glmaxx_kernel.h
clang++ -std=c++17 -x c++ \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null
clang -std=c11 -x c \
  -include docs/sm120-rank-executor-native-abi-v1.h \
  -fsyntax-only /dev/null

if command -v nvcc >/dev/null 2>&1; then
  echo "nvcc is present; CUDA execution still requires explicit cn4 authorization"
else
  echo "CUDA compile skipped: nvcc is not installed on this Phase A host"
fi
