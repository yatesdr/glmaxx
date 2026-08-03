#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 ABSOLUTE_TERMINAL_RUN_DIRECTORY" >&2
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

allocation_state="$(read_one_line "${run_dir}/allocation-state.txt")"
terminal_dir="${run_dir}/terminal-claim-v1"
if [[ -L "${terminal_dir}" || ! -d "${terminal_dir}" ||
      "$(read_one_line "${terminal_dir}/terminal-contract.txt")" != \
        "glmaxx-evidence-terminal-v1" ]]; then
  echo "evidence run has no valid terminal claim" >&2
  exit 70
fi
terminal_state="$(read_one_line "${terminal_dir}/terminal-state.txt")"
if [[ ("${terminal_state}" != "COMPLETE" && "${terminal_state}" != "FAILED") ||
      "${allocation_state}" != "${terminal_state}" ]]; then
  echo "allocation and terminal states are not one identical terminal value" >&2
  exit 70
fi
if [[ -e "${run_dir}/allocation-state.txt.tmp" ||
      -e "${run_dir}/evidence-sha256.txt.tmp" ]]; then
  echo "evidence run retains an unfinished state or manifest transaction" >&2
  exit 70
fi
if [[ -n "$(find "${run_dir}" -type l -print -quit)" ]]; then
  echo "evidence run contains a symlink" >&2
  exit 70
fi

manifest_path="${run_dir}/evidence-sha256.txt"
if [[ -L "${manifest_path}" || ! -s "${manifest_path}" ]]; then
  echo "evidence manifest is missing, unsafe, or empty" >&2
  exit 70
fi
if grep -Evq '^[0-9a-f]{64}  \./' "${manifest_path}"; then
  echo "evidence manifest contains a noncanonical line" >&2
  exit 70
fi

proof_dir="$(mktemp -d "${TMPDIR:-/tmp}/glmaxx-evidence-verify.XXXXXX")"
expected_paths="${proof_dir}/expected-paths.txt"
manifest_paths="${proof_dir}/manifest-paths.txt"
cleanup() {
  rm -f "${expected_paths}" "${manifest_paths}"
  rmdir "${proof_dir}"
}
trap cleanup EXIT

(
  cd "${run_dir}"
  find . -type f \
    ! -path './allocation-state.txt' \
    ! -path './evidence-sha256.txt' \
    -print | LC_ALL=C sort
) >"${expected_paths}"
sed -E 's/^[0-9a-f]{64}  //' "${manifest_path}" | LC_ALL=C sort \
  >"${manifest_paths}"
if ! cmp -s "${expected_paths}" "${manifest_paths}"; then
  echo "evidence manifest does not enumerate the exact sealed file set" >&2
  exit 70
fi
if ! (cd "${run_dir}" && shasum -a 256 -c evidence-sha256.txt >/dev/null); then
  echo "evidence content hash verification failed" >&2
  exit 70
fi

manifest_sha="$(shasum -a 256 "${manifest_path}" | awk '{print $1}')"
printf 'evidence-run-verify=pass state=%s files=%s manifest-sha256=%s run=%s\n' \
  "${terminal_state}" "$(wc -l <"${manifest_path}" | tr -d ' ')" \
  "${manifest_sha}" "${run_dir}"
