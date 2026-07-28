#!/usr/bin/env bash
set -euo pipefail

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "phase-b-authorized" ]]; then
  echo "Refusing GPU access: set GLMAXX_CN4_AUTHORIZATION=phase-b-authorized only after explicit operator authorization" >&2
  exit 64
fi

if [[ "${GLMAXX_REVIEW_GATE:-}" != "manifest-abi-v0.2.2-accepted" ]]; then
  echo "Refusing M2: independent review of the manifest and v0.2.2 ABI is not recorded" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

if [[ -z "${CUTLASS_DIR:-}" || -z "${GLMAXX_EVIDENCE_DIR:-}" || \
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

if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi

source_status="$(git status --porcelain)"
if [[ -n "${source_status}" ]]; then
  echo "Source tree must be committed before qualification" >&2
  exit 65
fi

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

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,driver_version,memory.total \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-before.csv"

check_idle

git rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
cmake --version | tee "${GLMAXX_EVIDENCE_DIR}/cmake.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
shasum -a 256 spec/engine-v0.md spec/format-v0.md \
  manifests/glm52-operation-v1.json benchmarks/sm120-fc1-matrix-v1.json \
  fixtures/sm120-fc1-matrix-proof-v1.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

cargo test --workspace --offline | tee "${GLMAXX_EVIDENCE_DIR}/cargo-test.txt"

build_dir="${GLMAXX_EVIDENCE_DIR}/build"
cmake -S kernels -B "${build_dir}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${build_dir}" --verbose \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"

"${build_dir}/glmaxx_cutlass_layout_probe" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-layout-probe.txt"

check_idle

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"

check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx \
  -- gpu-smoke 1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-smoke-m1.json"

correctness_dir="${GLMAXX_EVIDENCE_DIR}/correctness"
if [[ -e "${correctness_dir}" ]]; then
  echo "Correctness evidence directory already exists; refusing to overwrite it" >&2
  exit 65
fi
mkdir "${correctness_dir}"
check_idle
cargo run --release --offline -p glm-cli --features cuda-ffi --bin glmaxx \
  -- gpu-matrix "${correctness_dir}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-matrix-summary.json"

shasum -a 256 "${correctness_dir}"/*.json \
  | tee "${GLMAXX_EVIDENCE_DIR}/correctness-sha256.txt"
nvidia-smi \
  --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-after.csv"

echo "Correctness matrix finished. Do not benchmark unless summary.json reports 135 positive cases, 9 negative rejections, 2 deterministic cases, and zero failures."
