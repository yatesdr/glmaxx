#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 ABSOLUTE_RUNNING_RUN_DIRECTORY COMPLETE|FAILED" >&2
  exit 64
}

if [[ "$#" -ne 2 ]]; then
  usage
fi

run_input="$1"
terminal_state="$2"
case "${run_input}" in
  /*) ;;
  *)
    echo "run directory must be an absolute path" >&2
    exit 64
    ;;
esac
if [[ "${terminal_state}" != "COMPLETE" && "${terminal_state}" != "FAILED" ]]; then
  echo "terminal state must be COMPLETE or FAILED" >&2
  exit 64
fi
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

read_one_line() {
  local input_file="$1"
  local last_byte
  local line_count
  if [[ -L "${input_file}" || ! -f "${input_file}" ]]; then
    echo "missing or unsafe evidence state file: ${input_file}" >&2
    exit 70
  fi
  line_count="$(awk 'END { print NR }' "${input_file}")"
  last_byte="$(tail -c 1 "${input_file}")"
  if [[ "${line_count}" != "1" || -n "${last_byte}" ]]; then
    echo "evidence state file must contain exactly one newline-terminated line" >&2
    exit 70
  fi
  IFS= read -r input_value <"${input_file}"
  printf '%s' "${input_value}"
}

if [[ "$(read_one_line "${run_dir}/allocation-state.txt")" != "RUNNING" ]]; then
  echo "only a RUNNING evidence allocation can become terminal" >&2
  exit 70
fi
claim_dir="${run_dir}/runner-claim-v1"
if [[ -L "${claim_dir}" || ! -d "${claim_dir}" ||
      "$(read_one_line "${claim_dir}/runner-contract.txt")" != \
        "glmaxx-evidence-runner-v1" ]]; then
  echo "running evidence allocation has no valid runner claim" >&2
  exit 70
fi
if [[ -e "${run_dir}/allocation-state.txt.tmp" ]]; then
  echo "evidence run has a retained state transaction" >&2
  exit 70
fi

umask 077
terminal_dir="${run_dir}/terminal-claim-v1"
terminal_claimed=0

mark_incomplete() {
  if ((terminal_claimed == 1)); then
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

trap mark_incomplete ERR
trap 'stop_after_signal 129' HUP
trap 'stop_after_signal 130' INT
trap 'stop_after_signal 143' TERM

if ! mkdir "${terminal_dir}" 2>/dev/null; then
  echo "evidence run already has a terminal claim" >&2
  exit 75
fi
terminal_claimed=1

if [[ "$(read_one_line "${run_dir}/allocation-state.txt")" != "RUNNING" ]]; then
  echo "evidence allocation state changed while claiming terminal publication" >&2
  mark_incomplete
  trap - ERR HUP INT TERM
  exit 70
fi

terminal_clock="$(TZ=UTC date -u '+%Y%m%dT%H%M%SZ|%Y-%m-%dT%H:%M:%SZ|%s')"
IFS='|' read -r terminal_compact terminal_rfc3339 terminal_epoch <<<"${terminal_clock}"
if [[ ! "${terminal_compact}" =~ ^[0-9]{8}T[0-9]{6}Z$ ||
      ! "${terminal_rfc3339}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ||
      ! "${terminal_epoch}" =~ ^[0-9]+$ ]]; then
  echo "UTC clock returned an invalid terminal record" >&2
  mark_incomplete
  trap - ERR HUP INT TERM
  exit 70
fi

printf '%s\n' "glmaxx-evidence-terminal-v1" >"${terminal_dir}/terminal-contract.txt"
printf '%s\n' "${terminal_state}" >"${terminal_dir}/terminal-state.txt"
printf '%s\n' "${terminal_compact}" >"${terminal_dir}/terminal-compact-utc.txt"
printf '%s\n' "${terminal_rfc3339}" >"${terminal_dir}/terminal-utc.txt"
printf '%s\n' "${terminal_epoch}" >"${terminal_dir}/terminal-epoch-seconds.txt"

printf '%s\n' "${terminal_state}" >"${run_dir}/allocation-state.txt.tmp"
mv "${run_dir}/allocation-state.txt.tmp" "${run_dir}/allocation-state.txt"
trap - ERR HUP INT TERM

printf '%s\n' "${run_dir}"
