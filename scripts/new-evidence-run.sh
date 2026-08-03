#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 ABSOLUTE_EXISTING_ROOT RUN_SLUG" >&2
  exit 64
}

if [[ "$#" -ne 2 ]]; then
  usage
fi

root_input="$1"
run_slug="$2"

case "${root_input}" in
  /*) ;;
  *)
    echo "evidence root must be an absolute path" >&2
    exit 64
    ;;
esac

if [[ -L "${root_input}" || ! -d "${root_input}" ]]; then
  echo "evidence root must be an existing, non-symlink directory" >&2
  exit 65
fi

LC_ALL=C
export LC_ALL
if [[ ! "${run_slug}" =~ ^[a-z0-9][a-z0-9._-]{0,63}$ ]]; then
  echo "run slug must be 1-64 lowercase ASCII letters, digits, dots, underscores, or hyphens" >&2
  exit 64
fi

root_dir="$(cd "${root_input}" && pwd -P)"
if [[ "${root_dir}" == "/" ]]; then
  echo "filesystem root is not a valid evidence root" >&2
  exit 65
fi

# Capture the compact label, RFC 3339 receipt, and epoch from one clock read so
# the directory name cannot disagree with its recorded start time.
clock_record="$(TZ=UTC date -u '+%Y%m%dT%H%M%SZ|%Y-%m-%dT%H:%M:%SZ|%s')"
IFS='|' read -r compact_utc rfc3339_utc epoch_seconds <<<"${clock_record}"
if [[ ! "${compact_utc}" =~ ^[0-9]{8}T[0-9]{6}Z$ ||
      ! "${rfc3339_utc}" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ||
      ! "${epoch_seconds}" =~ ^[0-9]+$ ]]; then
  echo "UTC clock returned an invalid record" >&2
  exit 70
fi

umask 077
base_name="${compact_utc}-${run_slug}"
run_dir=""
allocation_sequence=""
for ((sequence = 0; sequence <= 99; sequence++)); do
  if ((sequence == 0)); then
    candidate_name="${base_name}"
  else
    printf -v suffix '%02d' "${sequence}"
    candidate_name="${base_name}-${suffix}"
  fi
  candidate_dir="${root_dir}/${candidate_name}"
  if mkdir "${candidate_dir}" 2>/dev/null; then
    run_dir="${candidate_dir}"
    allocation_sequence="${sequence}"
    break
  fi
  if [[ ! -e "${candidate_dir}" ]]; then
    echo "failed to create evidence directory: ${candidate_dir}" >&2
    exit 73
  fi
done

if [[ -z "${run_dir}" ]]; then
  echo "all 100 UTC evidence-directory candidates already exist" >&2
  exit 75
fi

mark_incomplete() {
  if [[ -n "${run_dir}" && -d "${run_dir}" ]]; then
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

printf '%s\n' "INCOMPLETE" >"${run_dir}/allocation-state.txt"
printf '%s\n' "glmaxx-evidence-run-v1" >"${run_dir}/allocator-contract.txt"
printf '%s\n' "${compact_utc}" >"${run_dir}/run-start-compact-utc.txt"
printf '%s\n' "${rfc3339_utc}" >"${run_dir}/run-start-utc.txt"
printf '%s\n' "${epoch_seconds}" >"${run_dir}/run-start-epoch-seconds.txt"
printf '%s\n' "${candidate_name}" >"${run_dir}/run-directory-basename.txt"
printf '%s\n' "${run_slug}" >"${run_dir}/run-slug.txt"
printf '%s\n' "${allocation_sequence}" >"${run_dir}/allocation-sequence.txt"

printf '%s\n' "READY" >"${run_dir}/allocation-state.txt.tmp"
mv "${run_dir}/allocation-state.txt.tmp" "${run_dir}/allocation-state.txt"
trap - ERR HUP INT TERM

printf '%s\n' "${run_dir}"
