#!/usr/bin/env bash
set -euo pipefail

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "rank-bind-authorized" ]]; then
  echo "Refusing CUDA context creation: set GLMAXX_CN4_AUTHORIZATION=rank-bind-authorized only after explicit operator authorization" >&2
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
if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != \
      "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before rank binding" >&2
  exit 65
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; no CUDA context was created" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$(
  nvidia-smi --query-gpu=compute_cap --format=csv,noheader \
    | tr -d '\r ' \
    | grep -c '^12\\.0$' || true
)"
if [[ "${gpu_count}" != "4" || "${sm120_count}" != "4" ]]; then
  echo "Rank binding requires exactly four visible compute-capability 12.0 devices" >&2
  exit 70
fi
check_idle

git rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
shasum -a 256 \
  crates/glm-cuda/src/abi.rs \
  crates/glm-cuda/src/ffi.rs \
  crates/glm-engine/src/worker.rs \
  crates/glm-cli/src/main.rs \
  kernels/include/glmaxx_kernel.h \
  kernels/sm120/nvfp4_routed_fc1.cu \
  scripts/cn4-rank-bind-smoke.sh \
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
nm -D --defined-only "${build_dir}/libglmaxx_sm120.so" \
  | grep -E 'glmaxx_device_(bind|count)' \
  | tee "${GLMAXX_EVIDENCE_DIR}/rank-bind-symbols.txt"
if [[ "$(wc -l < "${GLMAXX_EVIDENCE_DIR}/rank-bind-symbols.txt" | tr -d ' ')" != "2" ]]; then
  echo "Shared library must export exactly the device-count and device-bind controls" >&2
  exit 70
fi

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
cargo build --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-ffi-build.txt"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
check_idle
"${CARGO_TARGET_DIR}/release/glmaxx" gpu-rank-bind-smoke \
  | tee "${GLMAXX_EVIDENCE_DIR}/rank-bind.json"
if ! grep -Fq '"verdict": "SM120_TP4_RANK_BIND_PASS"' \
    "${GLMAXX_EVIDENCE_DIR}/rank-bind.json" ||
   ! grep -Fq '"kernel_launched": false' \
    "${GLMAXX_EVIDENCE_DIR}/rank-bind.json"; then
  echo "Rank binding did not emit the required pass record" >&2
  exit 70
fi
check_idle

shasum -a 256 \
  "${build_dir}/libglmaxx_sm120.so" \
  "${CARGO_TARGET_DIR}/release/glmaxx" \
  "${GLMAXX_EVIDENCE_DIR}/rank-bind.json" \
  | tee "${GLMAXX_EVIDENCE_DIR}/artifact-sha256.txt"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during rank binding" >&2
  exit 70
fi
printf '%s\n' \
  "SM120_TP4_RANK_BIND_PASS" \
  "All four persistent-rank test threads bound distinct compute-capability 12.0 devices and created, synchronized, and destroyed one nonblocking stream." \
  "No device kernel was launched." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
