#!/usr/bin/env bash
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
cd "$HOME/Modes/places.minilab.work"
export NODE_ENV=production
export PORT=4176
export ACTGRAPH_CAPITAL_URL="${ACTGRAPH_CAPITAL_URL:-http://127.0.0.1:7000}"
export ACTGRAPH_INFERENCE_URL="${ACTGRAPH_INFERENCE_URL:-http://127.0.0.1:8082}"
export ACTGRAPH_ACP_CMD="${ACTGRAPH_ACP_CMD:-$HOME/ActGraph/target/release/actgraph-acp}"
export ACTGRAPH_ACP_TIMEOUT_MS="${ACTGRAPH_ACP_TIMEOUT_MS:-120000}"
exec /opt/homebrew/bin/npm run start -- -p 4176 -H 127.0.0.1
