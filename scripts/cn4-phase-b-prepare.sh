#!/usr/bin/env bash
set -euo pipefail

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

if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != \
      "e05f953a5b3d38adc240df2ff928e0421c2abba3" ]]; then
  echo "CUTLASS revision mismatch" >&2
  exit 65
fi

source_status="$(git status --porcelain)"
if [[ -n "${source_status}" ]]; then
  echo "Source tree must be committed before qualification preparation" >&2
  exit 65
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}"

if command -v nvidia-smi >/dev/null 2>&1; then
  nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,driver_version,memory.total \
    --format=csv,noheader \
    | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory.csv"
  nvidia-smi topo -m | tee "${GLMAXX_EVIDENCE_DIR}/topology.txt"
  nvidia-smi \
    --query-gpu=index,uuid,clocks.current.sm,clocks.current.memory,power.limit,persistence_mode \
    --format=csv,noheader \
    | tee "${GLMAXX_EVIDENCE_DIR}/gpu-clocks-before.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader \
    > "${GLMAXX_EVIDENCE_DIR}/compute-apps-before.csv" || true
else
  printf '%s\n' "nvidia-smi unavailable; no device inventory captured" \
    | tee "${GLMAXX_EVIDENCE_DIR}/gpu-inventory-unavailable.txt"
fi

git rev-parse HEAD | tee "${GLMAXX_EVIDENCE_DIR}/source-commit.txt"
git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status.txt"
rustc --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/rustc.txt"
cargo --version --verbose | tee "${GLMAXX_EVIDENCE_DIR}/cargo.txt"
nvcc --version | tee "${GLMAXX_EVIDENCE_DIR}/nvcc.txt"
cmake --version | tee "${GLMAXX_EVIDENCE_DIR}/cmake.txt"
ninja --version | tee "${GLMAXX_EVIDENCE_DIR}/ninja.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/container-digest.txt"

shasum -a 256 \
  AGENTS.md \
  spec/engine-v0.md \
  spec/format-v0.md \
  manifests/glm52-operation-v1.json \
  benchmarks/sm120-fc1-matrix-v1.json \
  fixtures/sm120-fc1-matrix-proof-v1.json \
  fable-adversarial-v2.md \
  docs/fable-v2-disposition.md \
  containers/cn4-dev.Dockerfile \
  scripts/cn4-phase-b-prepare.sh \
  scripts/cn4-phase-b.sh \
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

"${build_dir}/glmaxx_cutlass_layout_probe" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-layout-probe.txt"
"${build_dir}/glmaxx_cutlass_activation_layout_probe" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-activation-layout-probe.txt"
cuobjdump --list-elf "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-elf.txt"
cuobjdump --dump-resource-usage "${build_dir}/libglmaxx_sm120.so" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuobjdump-resources.txt"
cuobjdump --dump-sass "${build_dir}/libglmaxx_sm120.so" \
  > "${GLMAXX_EVIDENCE_DIR}/glmaxx-library-sass.txt"
owned_omma_count="$(
  grep -c 'OMMA.SF.16864.F32.E2M1.E2M1.UE4M3.4X' \
    "${GLMAXX_EVIDENCE_DIR}/glmaxx-library-sass.txt" || true
)"
printf '%s\n' "${owned_omma_count}" \
  | tee "${GLMAXX_EVIDENCE_DIR}/glmaxx-owned-omma-count.txt"
if [[ "${owned_omma_count}" != "64" ]]; then
  echo "GLMAXX-owned dense control did not retain exactly 64 expected SM120 NVFP4 OMMA instructions" >&2
  exit 70
fi
nm -D --defined-only "${build_dir}/libglmaxx_sm120.so" \
  | grep 'glmaxx_nvfp4_dense_control_launch' \
  | tee "${GLMAXX_EVIDENCE_DIR}/glmaxx-dense-control-symbol.txt"
cuobjdump --list-elf "${build_dir}/glmaxx_cutlass_nvfp4_dense_control" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-dense-control-elf.txt"
cuobjdump --dump-resource-usage \
  "${build_dir}/glmaxx_cutlass_nvfp4_dense_control" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cutlass-dense-control-resources.txt"
cuobjdump --dump-sass "${build_dir}/glmaxx_cutlass_nvfp4_dense_control" \
  > "${GLMAXX_EVIDENCE_DIR}/cutlass-dense-control-sass.txt"

export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
cargo build --release --offline -p glm-cli --features cuda-ffi --bin glmaxx 2>&1 \
  | tee "${GLMAXX_EVIDENCE_DIR}/cargo-cuda-ffi-build.txt"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
ldd "${CARGO_TARGET_DIR}/release/glmaxx" \
  | tee "${GLMAXX_EVIDENCE_DIR}/cuda-ffi-linkage.txt"

shasum -a 256 \
  "${build_dir}/libglmaxx_sm120.so" \
  "${build_dir}/glmaxx_cutlass_layout_probe" \
  "${build_dir}/glmaxx_cutlass_activation_layout_probe" \
  "${build_dir}/glmaxx_cutlass_nvfp4_dense_control" \
  "${CARGO_TARGET_DIR}/release/glmaxx" \
  | tee "${GLMAXX_EVIDENCE_DIR}/build-artifact-sha256.txt"

git status --short --branch | tee "${GLMAXX_EVIDENCE_DIR}/source-status-after.txt"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Source tree changed during qualification preparation" >&2
  exit 70
fi

printf '%s\n' \
  "PREPARED_NO_DEVICE_LAUNCH" \
  "The sm_120f library, SFA/SFB probes, unlaunched CUTLASS dense control, and Rust FFI binary are built." \
  "No CUDA device kernel was launched by this script." \
  "An accepted manifest/v0.2.2 independent review remains mandatory before scripts/cn4-phase-b.sh." \
  | tee "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
