#!/usr/bin/env bash
set -euo pipefail

readonly expected_authorization="exl3-phase-b-authorized"
readonly expected_review_token="exl3-source-projection-v1-accepted"
readonly expected_review_relative="fable-exl3-source-projection-v1.md"
readonly expected_cutlass="e05f953a5b3d38adc240df2ff928e0421c2abba3"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing GPU access: set GLMAXX_CN4_AUTHORIZATION=${expected_authorization} only after explicit operator authorization" >&2
  exit 64
fi

if [[ "${GLMAXX_EXL3_REVIEW_GATE:-}" != "${expected_review_token}" ||
      -z "${GLMAXX_EXL3_REVIEW_ARTIFACT:-}" ]]; then
  echo "Refusing EXL3 Phase B: the exact review token and committed review artifact are required" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

if [[ -z "${CUTLASS_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" ||
      -z "${GLMAXX_CONTAINER_DIGEST:-}" ]]; then
  echo "CUTLASS_DIR, GLMAXX_EVIDENCE_DIR, and GLMAXX_CONTAINER_DIGEST are required" >&2
  exit 64
fi

if [[ ! "${GLMAXX_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "GLMAXX_CONTAINER_DIGEST must be a sha256:<64 lowercase hex> identity" >&2
  exit 64
fi

case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence directory must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist; use a fresh immutable path" >&2
  exit 65
fi

if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "${expected_cutlass}" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before EXL3 qualification" >&2
  exit 65
fi

if [[ ! -f "${GLMAXX_EXL3_REVIEW_ARTIFACT}" ]]; then
  echo "Review artifact does not exist or is not a regular file" >&2
  exit 65
fi
review_artifact="$(realpath "${GLMAXX_EXL3_REVIEW_ARTIFACT}")"
case "${review_artifact}" in
  "${repo_dir}"/*) ;;
  *)
    echo "Review artifact must be inside the source repository" >&2
    exit 65
    ;;
esac
review_relative="${review_artifact#"${repo_dir}/"}"
if [[ "${review_relative}" != "${expected_review_relative}" ]]; then
  echo "Review artifact must be the dedicated root ${expected_review_relative} result" >&2
  exit 65
fi
if ! git ls-files --error-unmatch "${review_relative}" >/dev/null 2>&1; then
  echo "Review artifact must be tracked by Git" >&2
  exit 65
fi
if ! grep -Fxq "${expected_review_token}" "${review_artifact}"; then
  echo "Review artifact does not contain the exact acceptance token" >&2
  exit 65
fi

require_hash() {
  local expected_sha="$1"
  local input_file="$2"
  local attestation_name="$3"
  local actual_sha
  actual_sha="$(shasum -a 256 "${input_file}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${expected_sha}" ]]; then
    echo "Reviewed input changed: ${input_file}" >&2
    exit 70
  fi
  if ! grep -Fxq "${attestation_name}-sha256=${expected_sha}" \
      "${review_artifact}"; then
    echo "Review artifact did not attest ${input_file}" >&2
    exit 70
  fi
}

# These four identities are the exact acceptance lines required by the
# independent handoff. Do not weaken this to a token-only gate.
require_hash \
  "7c5bfa0795aa6b78646b00b029f8d4edc7c15491d69ea19e423f770701b5cdc3" \
  "docs/exl3-trellis-cpu-contract.md" \
  "exl3-cpu-contract"
require_hash \
  "6a889c1987cbf9b0e69b8c99716acd753ad0626496a32d26a8b59135a17f22d7" \
  "docs/exl3-sm120-source-projection.md" \
  "exl3-sm120-design"
require_hash \
  "8b771eb88eac20dae28917faf3cf640b58c3b12baa6193b9720a89d8bc1538b1" \
  "crates/glm-format/src/exl3.rs" \
  "exl3-rust-oracle"
require_hash \
  "a50542774a585abeeb451c5248397da3b069296856ca8ae64423786ec5675857" \
  "kernels/sm120/exl3_projection_control.cu" \
  "exl3-cuda-control"

review_sha_before="$(shasum -a 256 "${review_artifact}" | awk '{print $1}')"
source_commit_before="$(git rev-parse HEAD)"

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no GPU work was launched" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-before.csv"

gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$(
  nvidia-smi --query-gpu=compute_cap --format=csv,noheader \
    | tr -d ' ' \
    | grep -c '^12\\.0$' || true
)"
if [[ "${gpu_count}" != "4" || "${sm120_count}" != "4" ]]; then
  echo "EXL3 Phase B requires exactly four visible compute-capability 12.0 GPUs" >&2
  exit 70
fi
check_idle

printf '%s\n' "${source_commit_before}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
cmake --version | tee "${GLMAXX_EVIDENCE_DIR}/cmake.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s  %s\n' "${review_sha_before}" "${review_relative}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/review-artifact-sha256.txt"
shasum -a 256 \
  docs/exl3-trellis-cpu-contract.md \
  docs/exl3-sm120-source-projection.md \
  crates/glm-format/src/exl3.rs \
  crates/glm-cuda/src/abi.rs \
  crates/glm-cuda/src/ffi.rs \
  crates/glm-cuda/src/ownership.rs \
  crates/glm-cuda/src/lib.rs \
  kernels/include/glmaxx_kernel.h \
  kernels/sm120/exl3_projection_control.cu \
  crates/glm-cli/src/main.rs \
  scripts/cn4-exl3-phase-b.sh \
  "${review_artifact}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_EVIDENCE_DIR}/cargo-target"
cargo test --workspace --offline 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"

build_dir="${GLMAXX_EVIDENCE_DIR}/kernel-build"
cmake -S kernels -B "${build_dir}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${build_dir}" --verbose 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"

cuobjdump --list-elf "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"
if ! grep -q 'sm_120' "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"; then
  echo "Kernel library does not contain an SM120 device image" >&2
  exit 70
fi
cuobjdump --dump-resource-usage "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-resources.txt"
cuobjdump --dump-sass "${build_dir}/libglmaxx_sm120.so" \
  > "${GLMAXX_EVIDENCE_DIR}/glmaxx-library-sass.txt"
nm -D --defined-only "${build_dir}/libglmaxx_sm120.so" \
  | grep -E 'glmaxx_exl3_(projection_launch|projection_workspace_bytes|kernel_abi)' \
  | tee "${GLMAXX_EVIDENCE_DIR}/glmaxx-exl3-symbols.txt"
if [[ "$(wc -l < "${GLMAXX_EVIDENCE_DIR}/glmaxx-exl3-symbols.txt" | tr -d ' ')" != "3" ]]; then
  echo "Shared library must export exactly the EXL3 launch, workspace, and ABI controls" >&2
  exit 70
fi

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
cargo build --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-ffi-build.txt"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
ldd "${CARGO_TARGET_DIR}/release/glmaxx" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuda-ffi-linkage.txt"
shasum -a 256 \
  "${build_dir}/libglmaxx_sm120.so" \
  "${CARGO_TARGET_DIR}/release/glmaxx" \
  | tee "${GLMAXX_EVIDENCE_DIR}/build-artifact-sha256.txt"

check_idle
"${CARGO_TARGET_DIR}/release/glmaxx" gpu-exl3-smoke gate 1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-gate-m1.json"
check_idle
"${CARGO_TARGET_DIR}/release/glmaxx" gpu-exl3-smoke up 1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-up-m1.json"
check_idle
"${CARGO_TARGET_DIR}/release/glmaxx" gpu-exl3-smoke down 1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-down-m1.json"

nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-after.csv"
shasum -a 256 \
  "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-gate-m1.json" \
  "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-up-m1.json" \
  "${GLMAXX_EVIDENCE_DIR}/gpu-exl3-down-m1.json" \
  | tee "${GLMAXX_EVIDENCE_DIR}/correctness-sha256.txt"

review_sha_after="$(shasum -a 256 "${review_artifact}" | awk '{print $1}')"
if [[ "${review_sha_after}" != "${review_sha_before}" ]]; then
  echo "Review artifact changed during qualification" >&2
  exit 70
fi
if [[ "$(git rev-parse HEAD)" != "${source_commit_before}" ||
      -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during qualification" >&2
  exit 70
fi

printf '%s\n' \
  "EXL3_SOURCE_PROJECTION_M1_CORRECTNESS_PASSED" \
  "Gate, up, and down M=1 source-order projections passed their CPU oracle and two-run determinism controls." \
  "This is a synthetic correctness gate, not a real-payload or performance result." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
