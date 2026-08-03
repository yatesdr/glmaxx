#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 ABSOLUTE_ALLOCATED_RUN_DIRECTORY" >&2
  exit 64
}

if [[ "$#" -ne 1 ]]; then
  usage
fi

run_input="$1"
case "${run_input}" in
  /*) ;;
  *)
    echo "run directory must be an absolute path" >&2
    exit 64
    ;;
esac
if [[ -L "${run_input}" || ! -d "${run_input}" ]]; then
  echo "run directory must be an existing, non-symlink directory" >&2
  exit 65
fi

LC_ALL=C
export LC_ALL
run_dir="$(cd "${run_input}" && pwd -P)"
if [[ "${run_dir}" == "/" ]]; then
  echo "filesystem root is not a valid run directory" >&2
  exit 65
fi

read_receipt() {
  local receipt_name="$1"
  local receipt_path="${run_dir}/${receipt_name}"
  local last_byte
  local line_count
  if [[ -L "${receipt_path}" || ! -f "${receipt_path}" ]]; then
    echo "missing or unsafe evidence receipt: ${receipt_name}" >&2
    exit 70
  fi
  line_count="$(awk 'END { print NR }' "${receipt_path}")"
  last_byte="$(tail -c 1 "${receipt_path}")"
  if [[ "${line_count}" != "1" || -n "${last_byte}" ]]; then
    echo "evidence receipt must contain exactly one newline-terminated line: ${receipt_name}" >&2
    exit 70
  fi
  IFS= read -r receipt_value <"${receipt_path}"
  printf '%s' "${receipt_value}"
}

contract="$(read_receipt allocator-contract.txt)"
state="$(read_receipt allocation-state.txt)"
compact_utc="$(read_receipt run-start-compact-utc.txt)"
rfc3339_utc="$(read_receipt run-start-utc.txt)"
epoch_seconds="$(read_receipt run-start-epoch-seconds.txt)"
recorded_basename="$(read_receipt run-directory-basename.txt)"
run_slug="$(read_receipt run-slug.txt)"
allocation_sequence="$(read_receipt allocation-sequence.txt)"
actual_basename="$(basename "${run_dir}")"

if [[ "${contract}" != "glmaxx-evidence-run-v1" || "${state}" != "READY" ]]; then
  echo "evidence run is not an unconsumed glmaxx-evidence-run-v1 allocation" >&2
  exit 70
fi
if [[ ! "${compact_utc}" =~ ^[0-9]{8}T[0-9]{6}Z$ ||
      ! "${rfc3339_utc}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ||
      ! "${epoch_seconds}" =~ ^[0-9]+$ ||
      ! "${run_slug}" =~ ^[a-z0-9][a-z0-9._-]{0,63}$ ||
      ! "${allocation_sequence}" =~ ^([0-9]|[1-9][0-9])$ ]]; then
  echo "evidence allocation receipt syntax is invalid" >&2
  exit 70
fi
if [[ "${compact_utc}" != "${rfc3339_utc//[-:]/}" ||
      "${recorded_basename}" != "${actual_basename}" ]]; then
  echo "evidence allocation identity receipts disagree" >&2
  exit 70
fi

expected_basename="${compact_utc}-${run_slug}"
if ((allocation_sequence > 0)); then
  printf -v allocation_suffix '%02d' "${allocation_sequence}"
  expected_basename="${expected_basename}-${allocation_suffix}"
fi
if [[ "${actual_basename}" != "${expected_basename}" ]]; then
  echo "evidence directory basename does not match its allocation sequence" >&2
  exit 70
fi
if [[ -e "${run_dir}/allocation-state.txt.tmp" ]]; then
  echo "evidence allocation has a retained state transaction" >&2
  exit 70
fi

umask 077
claim_dir="${run_dir}/runner-claim-v1"
claimed=0

mark_incomplete() {
  if ((claimed == 1)); then
    printf '%s\n' "INCOMPLETE" >"${run_dir}/allocation-state.txt.tmp" 2>/dev/null || true
    mv -f "${run_dir}/allocation-state.txt.tmp" \
      "${run_dir}/allocation-state.txt" 2>/dev/null || true
  fi
}

stop_after_signal() {
  local exit_code="$1"
  mark_incomplete
  trap - ERR HUP INT TERM
  exit "${exit_code}"
}

fail_claimed() {
  local exit_code="$1"
  local message="$2"
  echo "${message}" >&2
  mark_incomplete
  trap - ERR HUP INT TERM
  exit "${exit_code}"
}

trap mark_incomplete ERR
trap 'stop_after_signal 129' HUP
trap 'stop_after_signal 130' INT
trap 'stop_after_signal 143' TERM

if ! mkdir "${claim_dir}" 2>/dev/null; then
  echo "evidence run already has a runner claim" >&2
  exit 75
fi
claimed=1

# Recheck the mutable state after winning the atomic claim. A competing or
# stale consumer must never turn a non-READY run back into RUNNING.
if [[ "$(read_receipt allocation-state.txt)" != "READY" ]]; then
  fail_claimed 70 "evidence allocation state changed while claiming the run"
fi

begin_clock="$(TZ=UTC date -u '+%Y%m%dT%H%M%SZ|%Y-%m-%dT%H:%M:%SZ|%s')"
IFS='|' read -r begin_compact begin_rfc3339 begin_epoch <<<"${begin_clock}"
if [[ ! "${begin_compact}" =~ ^[0-9]{8}T[0-9]{6}Z$ ||
      ! "${begin_rfc3339}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ||
      ! "${begin_epoch}" =~ ^[0-9]+$ ]]; then
  fail_claimed 70 "UTC clock returned an invalid runner-begin record"
fi
printf '%s\n' "glmaxx-evidence-runner-v1" >"${claim_dir}/runner-contract.txt"
printf '%s\n' "${begin_compact}" >"${claim_dir}/runner-begin-compact-utc.txt"
printf '%s\n' "${begin_rfc3339}" >"${claim_dir}/runner-begin-utc.txt"
printf '%s\n' "${begin_epoch}" >"${claim_dir}/runner-begin-epoch-seconds.txt"

printf '%s\n' "RUNNING" >"${run_dir}/allocation-state.txt.tmp"
mv "${run_dir}/allocation-state.txt.tmp" "${run_dir}/allocation-state.txt"
trap - ERR HUP INT TERM

printf '%s\n' "${run_dir}"
