#!/usr/bin/env bash
set -euo pipefail

readonly expected_authorization="sm120-profiler-cycle-authorized"
readonly expected_origin="https://github.com/yatesdr/glmaxx.git"
readonly expected_cutlass="e05f953a5b3d38adc240df2ff928e0421c2abba3"
readonly expected_cuda="V13.3.33"
readonly expected_rust="rustc 1.92.0"

if [[ "${GLMAXX_CN4_AUTHORIZATION:-}" != "${expected_authorization}" ]]; then
  echo "Refusing cn4 use: set GLMAXX_CN4_AUTHORIZATION=${expected_authorization} only after explicit operator authorization" >&2
  exit 64
fi

for required in CUTLASS_DIR GLMAXX_CONTAINER_DIGEST GLMAXX_EVIDENCE_DIR GLMAXX_EXPECTED_COMMIT; do
  if [[ -z "${!required:-}" ]]; then
    echo "${required} is required" >&2
    exit 64
  fi
done
if [[ ! "${GLMAXX_CONTAINER_DIGEST}" =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "GLMAXX_CONTAINER_DIGEST must be an exact sha256 digest" >&2
  exit 64
fi
if [[ ! "${GLMAXX_EXPECTED_COMMIT}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "GLMAXX_EXPECTED_COMMIT must be a full lowercase Git commit" >&2
  exit 64
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"
case "${GLMAXX_EVIDENCE_DIR}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Evidence must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${GLMAXX_EVIDENCE_DIR}" ]]; then
  echo "Evidence path already exists; use a fresh immutable path" >&2
  exit 65
fi
build_root="${GLMAXX_BUILD_ROOT:-${GLMAXX_EVIDENCE_DIR}-build}"
case "${build_root}" in
  "${repo_dir}"|"${repo_dir}"/*)
    echo "Build root must be outside the Git repository" >&2
    exit 64
    ;;
esac
if [[ -e "${build_root}" ]]; then
  echo "Build root already exists; use a fresh path" >&2
  exit 65
fi

source_commit="$(git rev-parse HEAD)"
if [[ "${source_commit}" != "${GLMAXX_EXPECTED_COMMIT}" ]]; then
  echo "HEAD does not match GLMAXX_EXPECTED_COMMIT" >&2
  exit 65
fi
if [[ "$(git remote get-url origin)" != "${expected_origin}" ]]; then
  echo "Repository origin is not the pinned glmaxx origin" >&2
  exit 65
fi
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Tracked source and index must be clean" >&2
  exit 65
fi
while IFS= read -r untracked; do
  case "${untracked}" in
    docs/reviews/*) ;;
    *)
      echo "Unexpected untracked path: ${untracked}" >&2
      exit 65
      ;;
  esac
done < <(git ls-files --others --exclude-standard)
if [[ "$(git -C "${CUTLASS_DIR}" rev-parse HEAD)" != "${expected_cutlass}" ||
      -n "$(git -C "${CUTLASS_DIR}" status --porcelain)" ]]; then
  echo "CUTLASS checkout is not clean at the pinned revision" >&2
  exit 65
fi

review_envs=(
  GLMAXX_MANIFEST_REVIEW_ARTIFACT
  GLMAXX_EXL3_SOURCE_REVIEW_ARTIFACT
  GLMAXX_EXL3_WARP_REVIEW_ARTIFACT
  GLMAXX_NVFP4_FUSED_REVIEW_ARTIFACT
  GLMAXX_CURRENT_TREE_REVIEW_ARTIFACT
  GLMAXX_PROFILER_REVIEW_ARTIFACT
)
review_handoffs=(
  docs/fable-manifest-abi-v022-r2-handoff.md
  docs/fable-exl3-source-projection-v1-r2-handoff.md
  docs/fable-exl3-warp-decode-v2-r2-handoff.md
  docs/fable-nvfp4-fused-routed-moe-v1-r3-handoff.md
  docs/fable-current-tree-review-acceptance-v3-handoff.md
  docs/fable-sm120-profiler-package-v1-handoff.md
)
review_artifacts=()
for review_env in "${review_envs[@]}"; do
  review_artifact="${!review_env:-}"
  if [[ -z "${review_artifact}" || ! -f "${review_artifact}" ]]; then
    echo "${review_env} must name its committed review result" >&2
    exit 65
  fi
  review_artifact="$(realpath "${review_artifact}")"
  case "${review_artifact}" in
    "${repo_dir}"/*) ;;
    *)
      echo "Review results must be inside the repository" >&2
      exit 65
      ;;
  esac
  review_relative="${review_artifact#"${repo_dir}/"}"
  if ! git ls-files --error-unmatch "${review_relative}" >/dev/null 2>&1; then
    echo "Review result is not tracked: ${review_relative}" >&2
    exit 65
  fi
  review_artifacts+=("${review_artifact}")
done

required_tools=(cargo rustc cmake ninja nvcc nvidia-smi nsys ncu cuobjdump nvdisasm nm sha256sum)
for required_tool in "${required_tools[@]}"; do
  if ! command -v "${required_tool}" >/dev/null 2>&1; then
    echo "Required profiler/build tool is missing: ${required_tool}" >&2
    exit 69
  fi
done
if [[ "$(rustc --version)" != "${expected_rust}" ]]; then
  echo "Rust version mismatch" >&2
  exit 69
fi
if ! nvcc --version | grep -Fq "${expected_cuda}"; then
  echo "CUDA compiler version mismatch" >&2
  exit 69
fi
for ncu_section in LaunchStats Occupancy SpeedOfLight MemoryWorkloadAnalysis SchedulerStats WarpStateStats InstructionStats; do
  if ! ncu --list-sections | grep -Fq "${ncu_section}"; then
    echo "Nsight Compute section unavailable: ${ncu_section}" >&2
    exit 69
  fi
done
if ! ncu --help | grep -Fq -- "--nvtx-include" ||
   ! ncu --help | grep -Fq -- "--replay-mode" ||
   ! nsys profile --help | grep -Fq "cudaProfilerApi"; then
  echo "Installed Nsight tools lack the required capture/replay options" >&2
  exit 69
fi

gpu_count="$(nvidia-smi --query-gpu=index --format=csv,noheader | wc -l | tr -d ' ')"
sm120_count="$({ nvidia-smi --query-gpu=compute_cap --format=csv,noheader |
  tr -d '\r ' | grep -c '^12\.0$'; } || true)"
if [[ "${gpu_count}" != "4" || "${sm120_count}" != "4" ]]; then
  echo "Profiler cycle requires exactly four visible SM120 GPUs" >&2
  exit 70
fi
active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
if [[ -n "${active_pids//[[:space:]]/}" ]]; then
  echo "cn4 is occupied; no build or CUDA work was started" >&2
  exit 75
fi

mkdir -p "${GLMAXX_EVIDENCE_DIR}/preflight" \
  "${GLMAXX_EVIDENCE_DIR}/binaries" \
  "${build_root}/kernel" \
  "${build_root}/cargo-target"
preflight_dir="${GLMAXX_EVIDENCE_DIR}/preflight"
printf '%s\n' "${source_commit}" > "${preflight_dir}/source-commit.txt"
printf '%s\n' "${GLMAXX_CONTAINER_DIGEST}" > "${preflight_dir}/container-digest.txt"
printf '%s\n' "${build_root}" > "${preflight_dir}/build-root.txt"
git status --short --branch > "${preflight_dir}/source-status.txt"
git ls-files --others --exclude-standard > "${preflight_dir}/untracked-names.txt"
nvidia-smi --query-gpu=index,name,uuid,pci.bus_id,compute_cap,driver_version,memory.total,memory.free,clocks.current.sm,clocks.current.memory,power.draw,power.limit,temperature.gpu --format=csv,noheader > "${preflight_dir}/gpu-inventory.csv"
nvidia-smi topo -m > "${preflight_dir}/gpu-topology.txt"
rustc --version --verbose > "${preflight_dir}/rustc.txt"
cargo --version --verbose > "${preflight_dir}/cargo.txt"
cmake --version > "${preflight_dir}/cmake.txt"
ninja --version > "${preflight_dir}/ninja.txt"
nvcc --version > "${preflight_dir}/nvcc.txt"
nsys --version > "${preflight_dir}/nsys.txt"
ncu --version > "${preflight_dir}/ncu.txt"
ncu --list-sections > "${preflight_dir}/ncu-sections.txt"
git -C "${CUTLASS_DIR}" rev-parse HEAD > "${preflight_dir}/cutlass-commit.txt"
for required_tool in "${required_tools[@]}"; do
  tool_path="$(command -v "${required_tool}")"
  sha256sum "$(realpath "${tool_path}")"
done > "${preflight_dir}/tool-sha256.txt"

build_dir="${build_root}/kernel"
export CARGO_TARGET_DIR="${build_root}/cargo-target"
cmake -S kernels -B "${build_dir}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCUTLASS_DIR="${CUTLASS_DIR}" \
  > "${preflight_dir}/cmake-configure.txt" 2>&1
cmake --build "${build_dir}" --verbose \
  > "${preflight_dir}/cmake-build.txt" 2>&1
export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
cargo build --release --locked --offline -p glm-cli --features cuda-ffi --bin glmaxx \
  > "${preflight_dir}/cargo-build.txt" 2>&1
runner="${CARGO_TARGET_DIR}/release/glmaxx"
library="${build_dir}/libglmaxx_sm120.so"
if [[ ! -x "${runner}" || ! -f "${library}" ]]; then
  echo "Profiler runner or SM120 library was not produced" >&2
  exit 70
fi
cuobjdump --list-elf "${library}" > "${preflight_dir}/cuobjdump-elf.txt"
if ! grep -Fq "sm_120" "${preflight_dir}/cuobjdump-elf.txt"; then
  echo "Kernel library lacks an SM120 device image" >&2
  exit 70
fi
cuobjdump --dump-resource-usage "${library}" > "${preflight_dir}/cuobjdump-resources.txt"
cuobjdump --dump-sass "${library}" > "${preflight_dir}/library-sass.txt"
nm -D --defined-only "${library}" > "${preflight_dir}/library-symbols.txt"
for required_symbol in glmaxx_profiler_start glmaxx_profiler_stop glmaxx_nvtx_range_push glmaxx_nvtx_range_pop; do
  if ! grep -Fq "${required_symbol}" "${preflight_dir}/library-symbols.txt"; then
    echo "Profiler ABI symbol missing: ${required_symbol}" >&2
    exit 70
  fi
done
sha256sum "${runner}" "${library}" > "${preflight_dir}/build-sha256.txt"
cp -p "${runner}" "${GLMAXX_EVIDENCE_DIR}/binaries/glmaxx"
cp -p "${library}" "${GLMAXX_EVIDENCE_DIR}/binaries/libglmaxx_sm120.so"

export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
"${runner}" profile-plan "${preflight_dir}/profile-plan.json" \
  > "${preflight_dir}/profile-plan-command.txt"
"${runner}" profile-plan-validate "${preflight_dir}/profile-plan.json" \
  > "${preflight_dir}/profile-plan-validation.txt"
for index in "${!review_handoffs[@]}"; do
  "${runner}" review-proof "${review_handoffs[${index}]}" "${review_artifacts[${index}]}" \
    > "${preflight_dir}/review-$((index + 1)).json"
done
sha256sum "${review_artifacts[@]}" > "${preflight_dir}/review-sha256.txt"

active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
if [[ -n "${active_pids//[[:space:]]/}" ]]; then
  echo "cn4 became occupied during preflight; no GLMAXX kernel was launched" >&2
  exit 75
fi
if [[ "$(git rev-parse HEAD)" != "${source_commit}" ||
      -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Source identity drifted during preflight" >&2
  exit 70
fi

printf '%s\n' "PREFLIGHT_PASS_NO_DEVICE_LAUNCH" > "${preflight_dir}/verdict.txt"
printf '%s\n' "${runner}" > "${preflight_dir}/runner-path.txt"
printf '%s\n' "${library}" > "${preflight_dir}/library-path.txt"
echo "Profiler preflight passed; no CUDA kernel was launched"
