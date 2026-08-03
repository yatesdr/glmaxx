#!/usr/bin/env bash
set -euo pipefail

readonly expected_authorization="exl3-real-k3-authorized"
readonly expected_review_token="exl3-source-projection-v1-accepted"
readonly expected_review_relative="fable-exl3-source-projection-v1-r2.md"
readonly expected_cutlass="e05f953a5b3d38adc240df2ff928e0421c2abba3"
readonly expected_index_sha256="f5dcd976a64ca70808dd4d8bd3ad07e9610c8ca6c30e3a6ed77ddefdac4c1d21"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing GPU access: exact real-K3 authorization is required" >&2
  exit 64
fi
if [[ "${GLMAXX_EXL3_REVIEW_GATE:-}" != "${expected_review_token}" ||
      -z "${GLMAXX_EXL3_REVIEW_ARTIFACT:-}" ]]; then
  echo "Refusing real K3 replay: exact EXL3 review gate is required" >&2
  exit 64
fi
if [[ -z "${CUTLASS_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" ||
      -z "${GLMAXX_CONTAINER_DIGEST:-}" || -z "${GLMAXX_TR3_INDEX:-}" ]]; then
  echo "CUTLASS_DIR, GLMAXX_EVIDENCE_DIR, GLMAXX_CONTAINER_DIGEST, and GLMAXX_TR3_INDEX are required" >&2
  exit 64
fi
if [[ ! "${GLMAXX_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "GLMAXX_CONTAINER_DIGEST must be sha256:<64 lowercase hex>" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not exist" >&2
  exit 65
fi
if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "${expected_cutlass}" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed and clean" >&2
  exit 65
fi

index="$(realpath "${GLMAXX_TR3_INDEX}")"
if [[ ! -f "${index}" || "$(basename "${index}")" != "model.safetensors.index.json" ]]; then
  echo "GLMAXX_TR3_INDEX must name the real standard safetensors index" >&2
  exit 65
fi
index_sha256="$(shasum -a 256 "${index}" | awk '{print $1}')"
if [[ "${index_sha256}" != "${expected_index_sha256}" ]]; then
  echo "TR3 index identity mismatch" >&2
  exit 70
fi

review_artifact="$(realpath "${GLMAXX_EXL3_REVIEW_ARTIFACT}")"
case "${review_artifact}" in
  "${repo_dir}"/*) ;;
  *) echo "Review artifact must be inside the source repository" >&2; exit 65 ;;
esac
review_relative="${review_artifact#"${repo_dir}/"}"
if [[ "${review_relative}" != "${expected_review_relative}" ]] ||
   ! git ls-files --error-unmatch "${review_relative}" >/dev/null 2>&1 ||
   ! grep -Fxq "${expected_review_token}" "${review_artifact}"; then
  echo "Dedicated tracked EXL3 r2 acceptance result is required" >&2
  exit 65
fi

require_hash() {
  local expected_sha="$1"
  local input_file="$2"
  local attestation_name="$3"
  local actual_sha
  actual_sha="$(shasum -a 256 "${input_file}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${expected_sha}" ]] ||
     ! grep -Fxq "${attestation_name}-sha256=${expected_sha}" "${review_artifact}"; then
    echo "Reviewed EXL3 input mismatch: ${input_file}" >&2
    exit 70
  fi
}
require_hash \
  "7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3" \
  docs/exl3-trellis-cpu-contract.md exl3-cpu-contract
require_hash \
  "20e4f6007969d42d78aba586e0a2b2496fc19483cd94563ed366bcd3e1b2b389" \
  docs/exl3-sm120-source-projection.md exl3-sm120-design
require_hash \
  "c290960591295a1dd440818f97a7317fb42a061f9bc71c87a31b5bbb4cfd3647" \
  crates/glm-format/src/exl3.rs exl3-rust-oracle
require_hash \
  "241730ceaf629d01101629cb3f107e8d13fe92019444f4b635f9aa1d8cbc819d" \
  kernels/sm120/exl3_projection_control.cu exl3-cuda-control

review_sha_before="$(shasum -a 256 "${review_artifact}" | awk '{print $1}')"
source_commit_before="$(git rev-parse HEAD)"
mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no further GPU work was launched" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total \
  --format=csv,noheader | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$(nvidia-smi --query-gpu=compute_cap --format=csv,noheader | tr -d '\r ' | grep -c '^12\.0$' || true)"
printf 'visible_devices=%s sm120_devices=%s\n' "${gpu_count}" "${sm120_count}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-counts.txt"
if [[ "${gpu_count}" != 4 || "${sm120_count}" != 4 ]]; then
  echo "Real K3 replay requires exactly four visible SM120 GPUs" >&2
  exit 70
fi
check_idle

printf '%s\n' "${source_commit_before}" | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s  %s\n' "${index_sha256}" "${index}" | tee "${GLMAXX_EVIDENCE_DIR}/checkpoint-index-sha256.txt"
printf '%s  %s\n' "${review_sha_before}" "${review_relative}" | tee "${GLMAXX_EVIDENCE_DIR}/review-artifact-sha256.txt"
shasum -a 256 crates/glm-cli/src/main.rs scripts/cn4-exl3-real-k3-phase-c.sh \
  crates/glm-format/src/exl3.rs kernels/sm120/exl3_projection_control.cu \
  "${review_artifact}" | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_EVIDENCE_DIR}/cargo-target"
cargo test --workspace --offline 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"
build_dir="${GLMAXX_EVIDENCE_DIR}/kernel-build"
cmake -S kernels -B "${build_dir}" -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${build_dir}" --target glmaxx_sm120 --verbose 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"
cuobjdump --list-elf "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"
if ! grep -q 'exl3_projection_control.sm_120.cubin' "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"; then
  echo "EXL3 SM120 cubin is missing" >&2
  exit 70
fi
export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
cargo build --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-ffi-build.txt"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
runner="${CARGO_TARGET_DIR}/release/glmaxx"
shasum -a 256 "${runner}" "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/build-artifact-sha256.txt"

if "${runner}" gpu-exl3-real-k3-smoke "${index}" 3 6 0 gate 1 \
    > "${GLMAXX_EVIDENCE_DIR}/k4-negative-stdout.txt" \
    2> "${GLMAXX_EVIDENCE_DIR}/k4-negative-stderr.txt"; then
  echo "K=4 negative control unexpectedly passed the K=3-only command" >&2
  exit 70
fi
if ! grep -Fq 'Component("model.layers.3.mlp.experts.6.gate_proj.rank0.trellis")' \
    "${GLMAXX_EVIDENCE_DIR}/k4-negative-stderr.txt"; then
  echo "K=4 negative control did not fail at the expected source component" >&2
  exit 70
fi

for rank in 0 3; do
  for projection in gate up down; do
    check_idle
    output="${GLMAXX_EVIDENCE_DIR}/real-k3-layer3-expert0-rank${rank}-${projection}.json"
    "${runner}" gpu-exl3-real-k3-smoke "${index}" 3 0 "${rank}" "${projection}" 1 \
      | tee "${output}"
    grep -Fq '"bits": 3' "${output}"
    grep -Fq '"failed_elements": 0' "${output}"
    grep -Fq '"repeat_bitwise_deterministic": true' "${output}"
  done
done

nvidia-smi --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-after.csv"
shasum -a 256 "${GLMAXX_EVIDENCE_DIR}"/real-k3-*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/correctness-sha256.txt"
if [[ "$(shasum -a 256 "${review_artifact}" | awk '{print $1}')" != "${review_sha_before}" ||
      "$(git rev-parse HEAD)" != "${source_commit_before}" ||
      -n "$(git status --porcelain)" ]]; then
  echo "Source or review provenance changed during real K3 replay" >&2
  exit 70
fi
printf '%s\n' \
  "EXL3_REAL_TR3_K3_M1_CORRECTNESS_PASSED" \
  "Six real checkpoint projections matched the CPU oracle and repeated bitwise." \
  "K=4 remains fail-closed and is not accepted by this result." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
