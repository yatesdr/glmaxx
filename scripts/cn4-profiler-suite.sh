#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_dir}"

"${repo_dir}/scripts/cn4-profiler-preflight.sh"

build_root="${GLMAXX_BUILD_ROOT:-${GLMAXX_EVIDENCE_DIR}-build}"
build_dir="${build_root}/kernel"
runner="${build_root}/cargo-target/release/glmaxx"
export GLMAXX_KERNEL_LIB_DIR="${build_dir}"
export LD_LIBRARY_PATH="${build_dir}:${LD_LIBRARY_PATH:-}"
source_commit="$(git rev-parse HEAD)"

check_idle() {
  local active_pids
  active_pids="$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null || true)"
  if [[ -n "${active_pids//[[:space:]]/}" ]]; then
    echo "cn4 became occupied; refusing to overlap GPU work" >&2
    exit 75
  fi
}

snapshot_power() {
  local label="$1"
  printf 'snapshot=%s utc=%s\n' "${label}" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    >> "${GLMAXX_EVIDENCE_DIR}/gpu-power-clocks.txt"
  nvidia-smi --query-gpu=index,uuid,pstate,clocks.current.sm,clocks.current.memory,power.draw,power.limit,temperature.gpu,memory.used,memory.free --format=csv,noheader \
    >> "${GLMAXX_EVIDENCE_DIR}/gpu-power-clocks.txt"
}

mkdir -p "${GLMAXX_EVIDENCE_DIR}/correctness"
check_idle
snapshot_power correctness-before
"${runner}" gpu-rank-bind-smoke \
  > "${GLMAXX_EVIDENCE_DIR}/correctness/rank-bind.json"

mkdir "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-matrix"
check_idle
"${runner}" gpu-matrix "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-matrix" \
  > "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-matrix-command.txt"
mkdir "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-graph"
check_idle
"${runner}" gpu-graph "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-graph" \
  > "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-graph-command.txt"
mkdir "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-dense-control"
check_idle
"${runner}" gpu-dense-control "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-dense-control" \
  > "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-dense-command.txt"
mkdir "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-grouped-control"
check_idle
"${runner}" gpu-grouped-control "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-grouped-control" \
  > "${GLMAXX_EVIDENCE_DIR}/correctness/fc1-grouped-command.txt"
for rows in 1 256; do
  check_idle
  "${runner}" gpu-fc2-smoke "${rows}" \
    > "${GLMAXX_EVIDENCE_DIR}/correctness/fc2-m$(printf '%04d' "${rows}").json"
done
for projection in gate up down; do
  for rows in 1 8; do
    check_idle
    "${runner}" gpu-exl3-smoke "${projection}" "${rows}" \
      > "${GLMAXX_EVIDENCE_DIR}/correctness/exl3-${projection}-m$(printf '%04d' "${rows}").json"
  done
done
snapshot_power correctness-after

mkdir -p "${GLMAXX_EVIDENCE_DIR}/timing" "${GLMAXX_EVIDENCE_DIR}/commands"
mkdir "${GLMAXX_EVIDENCE_DIR}/timing/fc1-direct-host-enqueue"
check_idle
"${runner}" gpu-bench "${GLMAXX_EVIDENCE_DIR}/timing/fc1-direct-host-enqueue" \
  > "${GLMAXX_EVIDENCE_DIR}/commands/fc1-direct-host-enqueue.txt"
mkdir "${GLMAXX_EVIDENCE_DIR}/timing/fc1-grouped-host-enqueue"
check_idle
"${runner}" gpu-grouped-bench "${GLMAXX_EVIDENCE_DIR}/timing/fc1-grouped-host-enqueue" \
  > "${GLMAXX_EVIDENCE_DIR}/commands/fc1-grouped-host-enqueue.txt"
timing_rows=(1 128 256 3072)
grouped_routings=(empty-experts one-hot uniform zipf maximally-skewed)

run_timing_case() {
  local backend="$1"
  local mode="$2"
  local phase="$3"
  local routing="$4"
  local rows="$5"
  local case_id
  case_id="${backend}-${mode}-${phase}-${routing}-m$(printf '%04d' "${rows}")"
  local case_dir="${GLMAXX_EVIDENCE_DIR}/timing/${case_id}"
  mkdir "${case_dir}"
  check_idle
  "${runner}" gpu-time-case "${backend}" "${mode}" "${phase}" "${routing}" \
    "${rows}" 20 200 "${case_dir}" \
    > "${GLMAXX_EVIDENCE_DIR}/commands/timing-${case_id}.txt"
}

snapshot_power timing-before
for rows in "${timing_rows[@]}"; do
  for phase in quantize core inclusive; do
    run_timing_case nvfp4-direct-fc1 eager "${phase}" one-hot "${rows}"
  done
  run_timing_case nvfp4-direct-fc1 graph graph-inclusive one-hot "${rows}"
  for routing in "${grouped_routings[@]}"; do
    for phase in quantize core inclusive; do
      run_timing_case nvfp4-grouped-fc1 eager "${phase}" "${routing}" "${rows}"
    done
  done
  for phase in quantize core reduce inclusive; do
    run_timing_case nvfp4-direct-fc2 eager "${phase}" one-hot "${rows}"
  done
  for routing in "${grouped_routings[@]}"; do
    for phase in quantize core reduce inclusive; do
      run_timing_case nvfp4-grouped-fc2 eager "${phase}" "${routing}" "${rows}"
    done
  done
done
for backend in exl3-gate exl3-up exl3-down; do
  for rows in 1 8; do
    run_timing_case "${backend}" eager projection not-applicable "${rows}"
  done
done
snapshot_power timing-after

mkdir -p "${GLMAXX_EVIDENCE_DIR}/nsys" \
  "${GLMAXX_EVIDENCE_DIR}/ncu" \
  "${GLMAXX_EVIDENCE_DIR}/profile-case-output"

run_counter_case() {
  local backend="$1"
  local mode="$2"
  local phase="$3"
  local routing="$4"
  local rows="$5"
  local case_id
  case_id="${backend}-${mode}-${phase}-${routing}-m$(printf '%04d' "${rows}")"
  local nsys_target="${GLMAXX_EVIDENCE_DIR}/profile-case-output/nsys-${case_id}"
  local ncu_target="${GLMAXX_EVIDENCE_DIR}/profile-case-output/ncu-${case_id}"
  local nsys_base="${GLMAXX_EVIDENCE_DIR}/nsys/${case_id}"
  local ncu_base="${GLMAXX_EVIDENCE_DIR}/ncu/${case_id}"
  mkdir "${nsys_target}" "${ncu_target}"

  check_idle
  nsys profile \
    --trace=cuda,nvtx,osrt \
    --sample=none \
    --capture-range=cudaProfilerApi \
    --capture-range-end=stop \
    --force-overwrite=false \
    --output="${nsys_base}" \
    "${runner}" gpu-profile-case "${backend}" "${mode}" "${phase}" "${routing}" \
      "${rows}" 5 20 "${nsys_target}" \
    > "${GLMAXX_EVIDENCE_DIR}/commands/nsys-${case_id}.txt" 2>&1
  if [[ ! -s "${nsys_base}.nsys-rep" ]]; then
    echo "Nsight Systems report was not produced for ${case_id}" >&2
    exit 70
  fi
  nsys stats \
    --report cuda_gpu_kern_sum,cuda_api_sum,nvtx_sum \
    --format csv \
    "${nsys_base}.nsys-rep" \
    > "${nsys_base}-stats.csv"

  check_idle
  ncu \
    --target-processes all \
    --replay-mode kernel \
    --nvtx \
    --nvtx-include "glmaxx-profile/" \
    --section LaunchStats \
    --section Occupancy \
    --section SpeedOfLight \
    --section MemoryWorkloadAnalysis \
    --section SchedulerStats \
    --section WarpStateStats \
    --section InstructionStats \
    --force-overwrite false \
    --export "${ncu_base}" \
    "${runner}" gpu-profile-case "${backend}" "${mode}" "${phase}" "${routing}" \
      "${rows}" 5 1 "${ncu_target}" \
    > "${GLMAXX_EVIDENCE_DIR}/commands/ncu-${case_id}.txt" 2>&1
  if [[ ! -s "${ncu_base}.ncu-rep" ]]; then
    echo "Nsight Compute report was not produced for ${case_id}" >&2
    exit 70
  fi
  ncu --import "${ncu_base}.ncu-rep" --csv --page raw \
    > "${ncu_base}-raw.csv"
}

snapshot_power profiler-before
for rows in 1 3072; do
  for phase in quantize core inclusive; do
    run_counter_case nvfp4-direct-fc1 eager "${phase}" one-hot "${rows}"
    run_counter_case nvfp4-grouped-fc1 eager "${phase}" zipf "${rows}"
  done
  run_counter_case nvfp4-direct-fc1 graph graph-inclusive one-hot "${rows}"
  for phase in quantize core reduce inclusive; do
    run_counter_case nvfp4-direct-fc2 eager "${phase}" one-hot "${rows}"
    run_counter_case nvfp4-grouped-fc2 eager "${phase}" zipf "${rows}"
  done
done
for backend in exl3-gate exl3-up exl3-down; do
  for rows in 1 8; do
    run_counter_case "${backend}" eager projection not-applicable "${rows}"
  done
done
snapshot_power profiler-after

nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader \
  > "${GLMAXX_EVIDENCE_DIR}/gpu-processes-after.txt" || true
if [[ "$(git rev-parse HEAD)" != "${source_commit}" ||
      -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo "Source identity drifted during the profiler cycle" >&2
  exit 70
fi
printf '%s\n' "SM120_PROFILE_CYCLE_COMPLETE_NO_PERFORMANCE_CLAIM" \
  > "${GLMAXX_EVIDENCE_DIR}/verdict.txt"
"${runner}" profile-evidence-manifest "${GLMAXX_EVIDENCE_DIR}" "${source_commit}"
"${runner}" profile-evidence-validate "${GLMAXX_EVIDENCE_DIR}"
echo "SM120 profiler cycle completed and its artifact set validated"
