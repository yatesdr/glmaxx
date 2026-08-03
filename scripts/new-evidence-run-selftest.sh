#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
allocator="${repo_dir}/scripts/new-evidence-run.sh"
begin_runner="${repo_dir}/scripts/begin-evidence-run.sh"
finish_runner="${repo_dir}/scripts/finish-evidence-run.sh"

test_parent="${TMPDIR:-/tmp}"
test_root="$(mktemp -d "${test_parent%/}/glmaxx-evidence-selftest.XXXXXX")"

cleanup() {
  local base_name
  base_name="$(basename "${test_root}")"
  if [[ -n "${test_root}" && -d "${test_root}" &&
        "${test_root}" != "/" &&
        "${base_name}" == glmaxx-evidence-selftest.* ]]; then
    chmod -R u+w "${test_root}" 2>/dev/null || true
    rm -rf -- "${test_root}"
  fi
}
trap cleanup EXIT

evidence_root="${test_root}/evidence"
mkdir "${evidence_root}"
evidence_root="$(cd "${evidence_root}" && pwd -P)"

first_run="$(bash "${allocator}" "${evidence_root}" phase-b)"
second_run="$(bash "${allocator}" "${evidence_root}" phase-b)"

if [[ "${first_run}" == "${second_run}" ]]; then
  echo "allocator reused an evidence directory" >&2
  exit 1
fi

for run_dir in "${first_run}" "${second_run}"; do
  case "${run_dir}" in
    "${evidence_root}"/*) ;;
    *)
      echo "allocator returned a directory outside the evidence root" >&2
      exit 1
      ;;
  esac

  if [[ ! -d "${run_dir}" ]]; then
    echo "allocator did not create its returned directory" >&2
    exit 1
  fi
  if [[ "$(<"${run_dir}/allocation-state.txt")" != "READY" ]]; then
    echo "allocation did not reach READY state" >&2
    exit 1
  fi
  if [[ "$(<"${run_dir}/allocator-contract.txt")" != "glmaxx-evidence-run-v1" ]]; then
    echo "allocator contract receipt mismatch" >&2
    exit 1
  fi

  compact_utc="$(<"${run_dir}/run-start-compact-utc.txt")"
  rfc3339_utc="$(<"${run_dir}/run-start-utc.txt")"
  recorded_basename="$(<"${run_dir}/run-directory-basename.txt")"
  actual_basename="$(basename "${run_dir}")"
  compact_from_rfc3339="${rfc3339_utc//[-:]/}"

  if [[ "${compact_utc}" != "${compact_from_rfc3339}" ]]; then
    echo "compact and RFC 3339 UTC receipts disagree" >&2
    exit 1
  fi
  if [[ "${recorded_basename}" != "${actual_basename}" ||
        "${actual_basename}" != "${compact_utc}-phase-b"* ]]; then
    echo "directory basename disagrees with its UTC receipt" >&2
    exit 1
  fi
  if [[ ! "$(<"${run_dir}/run-start-epoch-seconds.txt")" =~ ^[0-9]+$ ]]; then
    echo "epoch receipt is invalid" >&2
    exit 1
  fi
  if [[ ! "$(<"${run_dir}/allocation-sequence.txt")" =~ ^([0-9]|[1-9][0-9])$ ]]; then
    echo "allocation sequence receipt is invalid" >&2
    exit 1
  fi
done

first_compact="$(<"${first_run}/run-start-compact-utc.txt")"
second_compact="$(<"${second_run}/run-start-compact-utc.txt")"
if [[ "${first_compact}" == "${second_compact}" &&
      "$(<"${first_run}/allocation-sequence.txt")" == \
        "$(<"${second_run}/allocation-sequence.txt")" ]]; then
  echo "same-second allocations did not receive distinct sequences" >&2
  exit 1
fi

concurrent_outputs="${test_root}/concurrent-outputs"
mkdir "${concurrent_outputs}"
worker_pids=()
for ((worker = 0; worker < 8; worker++)); do
  bash "${allocator}" "${evidence_root}" parallel \
    >"${concurrent_outputs}/${worker}.txt" &
  worker_pids+=("$!")
done
for worker_pid in "${worker_pids[@]}"; do
  wait "${worker_pid}"
done

begun_run="$(bash "${begin_runner}" "${first_run}")"
if [[ "${begun_run}" != "${first_run}" ||
      "$(<"${first_run}/allocation-state.txt")" != "RUNNING" ||
      "$(<"${first_run}/runner-claim-v1/runner-contract.txt")" != \
        "glmaxx-evidence-runner-v1" ]]; then
  echo "runner did not claim and begin the evidence allocation" >&2
  exit 1
fi
if bash "${begin_runner}" "${first_run}" >/dev/null 2>&1; then
  echo "runner replayed a consumed evidence allocation" >&2
  exit 1
fi
if [[ "$(<"${first_run}/allocation-state.txt")" != "RUNNING" ]]; then
  echo "runner replay changed the consumed evidence state" >&2
  exit 1
fi
bash "${finish_runner}" "${first_run}" COMPLETE >/dev/null
if [[ "$(<"${first_run}/allocation-state.txt")" != "COMPLETE" ||
      "$(<"${first_run}/terminal-claim-v1/terminal-contract.txt")" != \
        "glmaxx-evidence-terminal-v1" ||
      "$(<"${first_run}/terminal-claim-v1/terminal-state.txt")" != \
        "COMPLETE" ]]; then
  echo "runner did not publish the exact terminal state" >&2
  exit 1
fi
if bash "${finish_runner}" "${first_run}" FAILED >/dev/null 2>&1; then
  echo "terminal publication replayed a completed run" >&2
  exit 1
fi
if [[ "$(<"${first_run}/allocation-state.txt")" != "COMPLETE" ]]; then
  echo "terminal replay changed a completed run" >&2
  exit 1
fi

printf '%s\n' "tampered-basename" >"${second_run}/run-directory-basename.txt"
if bash "${begin_runner}" "${second_run}" >/dev/null 2>&1; then
  echo "runner accepted a tampered evidence receipt" >&2
  exit 1
fi
if [[ "$(<"${second_run}/allocation-state.txt")" != "READY" ||
      -e "${second_run}/runner-claim-v1" ]]; then
  echo "receipt rejection mutated the unclaimed allocation" >&2
  exit 1
fi

line_tamper="$(bash "${allocator}" "${evidence_root}" line-tamper)"
printf '%s\n%s' "READY" "TRAILING-DATA" >"${line_tamper}/allocation-state.txt"
if bash "${begin_runner}" "${line_tamper}" >/dev/null 2>&1; then
  echo "runner accepted a noncanonical single-line receipt" >&2
  exit 1
fi
if [[ -e "${line_tamper}/runner-claim-v1" ]]; then
  echo "noncanonical receipt rejection claimed the allocation" >&2
  exit 1
fi

invalid_terminal="$(bash "${allocator}" "${evidence_root}" invalid-terminal)"
bash "${begin_runner}" "${invalid_terminal}" >/dev/null
if bash "${finish_runner}" "${invalid_terminal}" SUCCESS >/dev/null 2>&1; then
  echo "terminal publisher accepted an invalid terminal state" >&2
  exit 1
fi
if [[ "$(<"${invalid_terminal}/allocation-state.txt")" != "RUNNING" ||
      -e "${invalid_terminal}/terminal-claim-v1" ]]; then
  echo "invalid terminal-state rejection mutated the running allocation" >&2
  exit 1
fi
bash "${finish_runner}" "${invalid_terminal}" FAILED >/dev/null

claim_race="$(bash "${allocator}" "${evidence_root}" claim-race)"
set +e
bash "${begin_runner}" "${claim_race}" \
  >"${test_root}/claim-race-0.out" 2>"${test_root}/claim-race-0.err" &
claim_pid_0="$!"
bash "${begin_runner}" "${claim_race}" \
  >"${test_root}/claim-race-1.out" 2>"${test_root}/claim-race-1.err" &
claim_pid_1="$!"
wait "${claim_pid_0}"
claim_status_0="$?"
wait "${claim_pid_1}"
claim_status_1="$?"
set -e
claim_successes=0
if [[ "${claim_status_0}" == "0" ]]; then
  ((claim_successes += 1))
fi
if [[ "${claim_status_1}" == "0" ]]; then
  ((claim_successes += 1))
fi
if [[ "${claim_successes}" != "1" ||
      "$(<"${claim_race}/allocation-state.txt")" != "RUNNING" ]]; then
  echo "concurrent runner claim did not select exactly one consumer" >&2
  exit 1
fi
bash "${finish_runner}" "${claim_race}" FAILED >/dev/null
if [[ "$(<"${claim_race}/allocation-state.txt")" != "FAILED" ]]; then
  echo "runner failure did not publish FAILED" >&2
  exit 1
fi

finish_race="$(bash "${allocator}" "${evidence_root}" finish-race)"
bash "${begin_runner}" "${finish_race}" >/dev/null
set +e
bash "${finish_runner}" "${finish_race}" COMPLETE \
  >"${test_root}/finish-race-0.out" 2>"${test_root}/finish-race-0.err" &
finish_pid_0="$!"
bash "${finish_runner}" "${finish_race}" FAILED \
  >"${test_root}/finish-race-1.out" 2>"${test_root}/finish-race-1.err" &
finish_pid_1="$!"
wait "${finish_pid_0}"
finish_status_0="$?"
wait "${finish_pid_1}"
finish_status_1="$?"
set -e
finish_successes=0
if [[ "${finish_status_0}" == "0" ]]; then
  ((finish_successes += 1))
fi
if [[ "${finish_status_1}" == "0" ]]; then
  ((finish_successes += 1))
fi
if [[ "${finish_successes}" != "1" ]]; then
  echo "concurrent terminal publication did not select exactly one winner" >&2
  exit 1
fi
finish_state="$(<"${finish_race}/allocation-state.txt")"
finish_receipt="$(<"${finish_race}/terminal-claim-v1/terminal-state.txt")"
if [[ "${finish_state}" != "${finish_receipt}" ||
      ("${finish_state}" != "COMPLETE" && "${finish_state}" != "FAILED") ]]; then
  echo "winning terminal publication disagrees with its state receipt" >&2
  exit 1
fi

unique_concurrent="$({
  for ((worker = 0; worker < 8; worker++)); do
    sed -n '1p' "${concurrent_outputs}/${worker}.txt"
  done
} | sort -u | wc -l | tr -d ' ')"
if [[ "${unique_concurrent}" != "8" ]]; then
  echo "concurrent allocations were not unique" >&2
  exit 1
fi

for ((worker = 0; worker < 8; worker++)); do
  concurrent_run="$(<"${concurrent_outputs}/${worker}.txt")"
  case "${concurrent_run}" in
    "${evidence_root}"/*) ;;
    *)
      echo "concurrent allocator escaped the evidence root" >&2
      exit 1
      ;;
  esac
  if [[ "$(<"${concurrent_run}/allocation-state.txt")" != "READY" ||
        "$(<"${concurrent_run}/run-slug.txt")" != "parallel" ||
        "$(<"${concurrent_run}/run-directory-basename.txt")" != \
          "$(basename "${concurrent_run}")" ]]; then
    echo "concurrent allocation receipt mismatch" >&2
    exit 1
  fi
done

if bash "${allocator}" "${evidence_root}" 'Bad Slug' >/dev/null 2>&1; then
  echo "allocator accepted an invalid slug" >&2
  exit 1
fi

ln -s "${evidence_root}" "${test_root}/evidence-link"
if bash "${allocator}" "${test_root}/evidence-link" phase-b >/dev/null 2>&1; then
  echo "allocator accepted a symlink evidence root" >&2
  exit 1
fi

printf '%s\n' "not a directory" >"${test_root}/regular-file"
if bash "${allocator}" "${test_root}/regular-file" phase-b >/dev/null 2>&1; then
  echo "allocator accepted a regular-file evidence root" >&2
  exit 1
fi

if bash "${allocator}" relative/evidence phase-b >/dev/null 2>&1; then
  echo "allocator accepted a relative evidence root" >&2
  exit 1
fi

if bash "${allocator}" / phase-b >/dev/null 2>&1; then
  echo "allocator accepted the filesystem root" >&2
  exit 1
fi

printf '%s\n' "evidence-run-selftest=pass allocations=14 concurrent=8 runner-claims=4 terminal-claims=4 rejection-cases=10"
