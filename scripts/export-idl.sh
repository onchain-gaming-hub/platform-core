#!/usr/bin/env bash
# Export the IDL that consumers use with `declare_program!(platform_core)`.
# declare_program! reads `idls/<program_name>.json` from the nearest ancestor `idls/` dir,
# so consumers copy this file to their own `idls/platform_core.json`.
# Run after `anchor build`; tests/test_idl_export.rs fails if this copy drifts.
set -euo pipefail
cd "$(dirname "$0")/.."
cp target/idl/platform_core.json idls/platform_core.json
echo "exported idls/platform_core.json ($(wc -c < idls/platform_core.json) bytes)"
