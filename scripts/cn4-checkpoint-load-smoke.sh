#!/usr/bin/env bash
set -euo pipefail

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "checkpoint-load-authorized" ]]; then
  echo "Refusing checkpoint load: set GLMAXX_CN4_AUTHORIZATION=checkpoint-load-authorized only after renewed operator authorization" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

required_variables=(
  CUTLASS_DIR
  GLMAXX_CONTAINER_DIGEST
  GLMAXX_EVIDENCE_DIR
  GLMAXX_PROFILE_BUDGET
  GLMAXX_RANK_SET_DIR
)
for variable in "${required_variables[@]}"; do
  if [[ -z "${!variable:-}" ]]; then
    echo "${variable} is required" >&2
    exit 64
  fi
done
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
case "${GLMAXX_RANK_SET_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Checkpoint weights must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence directory must not already exist; use a fresh immutable path" >&2
  exit 65
fi
if [[ ! -d "${GLMAXX_RANK_SET_DIR}" || ! -f "${GLMAXX_PROFILE_BUDGET}" ]]; then
  echo "Rank-set directory and completed profile-budget file must already exist" >&2
  exit 65
fi
if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != \
      "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree must be committed before checkpoint loading" >&2
  exit 65
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 is occupied; checkpoint load was not started or did not release every context" >&2
    printf '%s\n' "${active_pids}" > "${GLMAXX_EVIDENCE_DIR}/occupied-pids.txt"
    exit 75
  fi
}

nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total,memory.free,memory.used,compute_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory-before.csv"
nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$(
  nvidia-smi --query-gpu=compute_cap --format=csv,noheader \
    | tr -d '\r ' \
    | grep -c '^12\.0$' || true
)"
printf 'visible_devices=%s sm120_devices=%s\n' "${gpu_count}" "${sm120_count}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-counts.txt"
if [[ "${gpu_count}" != "4" || "${sm120_count}" != "4" ]]; then
  echo "Checkpoint load requires exactly four visible compute-capability 12.0 devices" >&2
  exit 70
fi
check_idle

source_commit="$(git rev-parse HEAD)"
printf '%s\n' "${source_commit}" | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
nvidia-smi --query-gpu=driver_version --format=csv,noheader \
  | sort -u | tee "${GLMAXX_EVIDENCE_DIR}/driver-versions.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"
printf '%s\n' "${GLMAXX_RANK_SET_DIR}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/rank-set-path.txt"
printf '%s\n' "${GLMAXX_PROFILE_BUDGET}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/profile-budget-path.txt"
stat --printf='%n size=%s device=%d inode=%i mtime=%Y links=%h\n' \
  "${GLMAXX_RANK_SET_DIR}"/rank-{0,1,2,3}.g5n \
  | tee "${GLMAXX_EVIDENCE_DIR}/rank-file-stat.txt"
sha256sum "${GLMAXX_PROFILE_BUDGET}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/profile-budget-sha256.txt"
sha256sum \
  crates/glm-cuda/src/ffi.rs \
  crates/glm-engine/src/checkpoint_cuda.rs \
  crates/glm-engine/src/checkpoint_load.rs \
  crates/glm-engine/src/native_worker.rs \
  crates/glm-engine/src/worker.rs \
  crates/glm-cli/src/main.rs \
  kernels/include/glmaxx_kernel.h \
  kernels/sm120/exl3_projection_control.cu \
  kernels/sm120/nvfp4_routed_fc1.cu \
  scripts/cn4-checkpoint-load-smoke.sh \
  | tee "${GLMAXX_EVIDENCE_DIR}/input-sha256.txt"

export CARGO_TARGET_DIR="${GLMAXX_EVIDENCE_DIR}/cargo-target"
scripts/local-checks.sh 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/local-checks.txt"

build_dir="${GLMAXX_EVIDENCE_DIR}/kernel-build"
cmake -S kernels -B "${build_dir}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCUTLASS_DIR="${CUTLASS_DIR}" 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-configure.txt"
cmake --build "${build_dir}" --verbose 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cmake-build.txt"

required_symbols=(
  glmaxx_device_alloc
  glmaxx_device_bind
  glmaxx_device_count
  glmaxx_device_free
  glmaxx_device_memory_info
  glmaxx_exl3_kernel_abi
  glmaxx_exl3_projection_workspace_bytes
  glmaxx_kernel_abi
  glmaxx_memcpy_d2h
  glmaxx_memcpy_h2d
  glmaxx_memset_zero
  glmaxx_pinned_alloc
  glmaxx_pinned_free
  glmaxx_stream_create
  glmaxx_stream_destroy
  glmaxx_stream_synchronize
)
nm -D --defined-only "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/native-symbols.txt"
for symbol in "${required_symbols[@]}"; do
  if ! grep -Eq "[[:space:]]${symbol}$" "${GLMAXX_EVIDENCE_DIR}/native-symbols.txt"; then
    echo "Native checkpoint library is missing ${symbol}" >&2
    exit 70
  fi
done

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
export GLMAXX_SOURCE_COMMIT="${source_commit}"
cargo build --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-ffi-build.txt"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
check_idle

checkpoint_evidence="${GLMAXX_EVIDENCE_DIR}/checkpoint-load"
mkdir "${checkpoint_evidence}"
"${CARGO_TARGET_DIR}/release/glmaxx" gpu-checkpoint-load-smoke \
  "${GLMAXX_RANK_SET_DIR}" \
  "${GLMAXX_PROFILE_BUDGET}" \
  "${checkpoint_evidence}" \
  900 2>&1 | tee "${GLMAXX_EVIDENCE_DIR}/checkpoint-load.txt"

summary="${checkpoint_evidence}/summary.json"
if [[ ! -f "${summary}" ]] ||
   ! grep -Fq '"verdict": "SM120_TP4_CHECKPOINT_LOAD_PASS"' "${summary}" ||
   ! grep -Fq '"model_kernel_launched": false' "${summary}" ||
   [[ "$(grep -c '"cleanup_acknowledged": true' "${summary}")" != "4" ]]; then
  echo "Checkpoint-load summary did not satisfy the success and cleanup contract" >&2
  exit 70
fi

check_idle
nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total,memory.free,memory.used,compute_mode \
  --format=csv,noheader \
  | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory-after.csv"
sha256sum \
  "${build_dir}/libglmaxx_sm120.so" \
  "${CARGO_TARGET_DIR}/release/glmaxx" \
  "${checkpoint_evidence}/memory-plan.json" \
  "${summary}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/artifact-sha256.txt"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during checkpoint load" >&2
  exit 70
fi
printf '%s\n' \
  "SM120_TP4_CHECKPOINT_LOAD_PASS" \
  "All four rank files passed full payload SHA-256 verification, HBM upload, full arena readback, global adoption, and rank-exact cleanup." \
  "No model execution kernel was launched." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
