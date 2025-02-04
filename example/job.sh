#!/bin/bash
set -ex -o pipefail

TOOL_PATH=${TOOL_PATH:-/usr/src/}

mkdir -p tmp

report_path=tmp/report.json
gitleaks git --no-color --no-banner --exit-code=0 --verbose \
  --report-format template --report-template "${TOOL_PATH}/jsonextra.json.tmpl" \
  --report-path "${report_path}" -c "${TOOL_PATH}/scan_config.toml"

gls apply --no-fail -c "${TOOL_PATH}/empty_allowlist.toml" -r "${report_path}" -f sarif | \
  reviewdog -f=sarif -fail-on-error -name="${JOB_NAME}" -reporter=github-pr-review || \
  # Send notification etc.
  echo "scan failed"
