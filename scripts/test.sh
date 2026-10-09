#!/usr/bin/env bash
# Full local test run. Tests load PRE-BUILT .so files, so always build first.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
anchor build
# TEST ONLY consumer program (never deployed) — reads idls/platform_core.json.
cargo build-sbf --manifest-path tests/mock-consumer/Cargo.toml --sbf-out-dir target/deploy
# build-sbf generates a keypair that does NOT match declare_id! — remove it so nothing
# can ever be deployed from it by accident.
rm -f target/deploy/mock_consumer-keypair.json
cargo test -p platform_core "$@"
