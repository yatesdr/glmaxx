#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 ABSOLUTE_EXISTING_EVIDENCE_ROOT prepare|qualify" >&2
  exit 64
}

if [[ "$#" -ne 2 ]]; then
  usage
fi
if [[ -n "${GLMAXX_EVIDENCE_DIR:-}" ]]; then
  echo "GLMAXX_EVIDENCE_DIR must be unset; this wrapper owns it" >&2
  exit 64
fi

evidence_root="$1"
phase_mode="$2"
repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
case "${phase_mode}" in
  prepare)
    run_slug="phase-b-prepare"
    phase_script="${repo_dir}/scripts/cn4-phase-b-prepare.sh"
    ;;
  qualify)
    run_slug="phase-b-qualification"
    phase_script="${repo_dir}/scripts/cn4-phase-b.sh"
    ;;
  *) usage ;;
esac

run_dir="$(bash "${repo_dir}/scripts/new-evidence-run.sh" \
  "${evidence_root}" "${run_slug}")"
bash "${repo_dir}/scripts/begin-evidence-run.sh" "${run_dir}" >/dev/null
terminal_published=0
publish_failure() {
  local exit_code="$1"
  trap - ERR HUP INT TERM
  if ((terminal_published == 0)); then
    bash "${repo_dir}/scripts/finish-evidence-run.sh" \
      "${run_dir}" FAILED >/dev/null 2>&1 || true
    terminal_published=1
  fi
  exit "${exit_code}"
}
trap 'publish_failure $?' ERR
trap 'publish_failure 129' HUP
trap 'publish_failure 130' INT
trap 'publish_failure 143' TERM

payload_dir="${run_dir}/payload"
if [[ -e "${payload_dir}" ]]; then
  echo "allocated evidence payload path unexpectedly exists" >&2
  publish_failure 70
fi

runner_sha="$(shasum -a 256 "${phase_script}" | awk '{print $1}')"
printf '%s\n' "${phase_mode}" >"${run_dir}/runner-claim-v1/phase-b-mode.txt"
printf '%s  %s\n' "${runner_sha}" "${phase_script#"${repo_dir}/"}" \
  >"${run_dir}/runner-claim-v1/phase-b-script-sha256.txt"
printf 'GLMAXX_RUN_DIR=%s\n' "${run_dir}"

if GLMAXX_EVIDENCE_DIR="${payload_dir}" bash "${phase_script}"; then
  phase_status=0
else
  phase_status="$?"
fi

if [[ "${phase_status}" == "0" ]]; then
  terminal_state="COMPLETE"
else
  terminal_state="FAILED"
fi
bash "${repo_dir}/scripts/finish-evidence-run.sh" \
  "${run_dir}" "${terminal_state}" >/dev/null
terminal_published=1
trap - ERR HUP INT TERM

printf 'GLMAXX_RUN_STATE=%s\n' "${terminal_state}"
exit "${phase_status}"
