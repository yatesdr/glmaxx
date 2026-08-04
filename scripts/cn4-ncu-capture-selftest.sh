#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/glmaxx-ncu-capture-proof.XXXXXX")"
trap 'rm -rf "${test_root}"' EXIT

mkdir "${test_root}/bin"
mock_ncu="${test_root}/bin/ncu"
capture="${test_root}/arguments.bin"
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  ': "${GLMAXX_NCU_ARGUMENT_CAPTURE:?}"' \
  'printf '\''%s\0'\'' "$@" > "${GLMAXX_NCU_ARGUMENT_CAPTURE}"' \
  > "${mock_ncu}"
chmod +x "${mock_ncu}"

PATH="${test_root}/bin:${PATH}" \
GLMAXX_NCU_ARGUMENT_CAPTURE="${capture}" \
  "${repo_dir}/scripts/cn4-ncu-capture.sh" \
  "${test_root}/fresh-report" \
  /opt/glmaxx/bin/glmaxx gpu-profile-case exl3-gate eager projection \
  not-applicable 1 5 1 "${test_root}/output"

arguments=()
while IFS= read -r -d '' argument; do
  arguments+=("${argument}")
done < "${capture}"

expected=(
  --target-processes all
  --replay-mode kernel
  --nvtx
  --nvtx-include glmaxx-profile/
  --section LaunchStats
  --section Occupancy
  --section SpeedOfLight
  --section MemoryWorkloadAnalysis
  --section SchedulerStats
  --section WarpStateStats
  --section InstructionStats
  --export "${test_root}/fresh-report"
  /opt/glmaxx/bin/glmaxx gpu-profile-case exl3-gate eager projection
  not-applicable 1 5 1 "${test_root}/output"
)

if [[ "${#arguments[@]}" != "${#expected[@]}" ]]; then
  echo "ncu argument count mismatch: expected ${#expected[@]}, observed ${#arguments[@]}" >&2
  exit 70
fi
for index in "${!expected[@]}"; do
  if [[ "${arguments[index]}" != "${expected[index]}" ]]; then
    echo "ncu argument ${index} mismatch: expected '${expected[index]}', observed '${arguments[index]}'" >&2
    exit 70
  fi
done
for argument in "${arguments[@]}"; do
  if [[ "${argument}" == "false" || "${argument}" == --force-overwrite* ]]; then
    echo "unsafe Nsight Compute overwrite argument escaped into the command" >&2
    exit 70
  fi
done

echo "cn4-ncu-capture-selftest=pass arguments=${#arguments[@]} overwrite-option=omitted target-boundary=exact"
