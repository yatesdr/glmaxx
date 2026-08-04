#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -lt 2 || -z "$1" ]]; then
  echo "usage: cn4-ncu-capture.sh report-base target [target-argument ...]" >&2
  exit 64
fi

report_base="$1"
shift

# The evidence allocator guarantees a fresh report path. Nsight Compute 2026.2
# defines --force-overwrite as a valueless flag; spelling it as
# `--force-overwrite false` executes `false` as the profile target. Omit the
# option so an unexpected pre-existing report remains a fail-closed error.
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
  --export "${report_base}" \
  "$@"
