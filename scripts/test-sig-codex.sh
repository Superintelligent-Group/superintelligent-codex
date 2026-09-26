#!/usr/bin/env bash
# Integrated test gate for the SIG Codex patch stack.
#
# Runs every test suite a patch in patches/codex/stack.json touches, against
# the fork checkout on its stack branch, with a dedicated target dir. Worktrees
# must not share a target dir: cargo reuses outputs across checkouts and can run
# stale code (seen in practice), so results from a shared dir are not evidence.
#
# usage: scripts/test-sig-codex.sh [fork-checkout] [target-dir]
set -uo pipefail

FORK="${1:-C:/Github/superintelligent-codex}"
TARGET="${2:-C:/cargo-target/sig-codex-int}"
LOG_DIR="${TARGET}/sig-test-logs"
mkdir -p "$LOG_DIR"

# Git for Windows ships a coreutils link.exe that shadows the MSVC linker.
PATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/usr/bin$' | paste -sd:):/usr/bin"
export PATH CARGO_TARGET_DIR="$TARGET" RUST_MIN_STACK=33554432

cd "$FORK/codex-rs" || exit 2
echo "fork $(git rev-parse --abbrev-ref HEAD) @ $(git rev-parse --short HEAD)"

# name | cargo test arguments
SUITES=(
  "cli-check|check -p codex-cli --bin codex"
  "codex-api|test -p codex-api"
  "core-lib|test -p codex-core --lib"
  "core-network|test -p codex-core --test all -- websocket_fallback client_websockets models_cache_ttl injected_models_cache"
  "models-manager|test -p codex-models-manager"
  "app-server-account|test -p codex-app-server --test all -- suite::v2::account"
  "daemon|test -p codex-app-server-daemon"
  "rmcp-windows|test -p codex-rmcp-client --test stdio_message_limits"
  "core-plugins|test -p codex-core-plugins"
  "skills|test -p codex-skills-extension"
  "state|test -p codex-state"
)

failed=()
for suite in "${SUITES[@]}"; do
  name="${suite%%|*}"; args="${suite#*|}"
  start=$(date +%s)
  # shellcheck disable=SC2086
  if cargo $args >"$LOG_DIR/$name.log" 2>&1; then status=PASS; else status=FAIL; failed+=("$name"); fi
  summary=$(grep -E '^test result:' "$LOG_DIR/$name.log" | awk '{p+=$4; f+=$6} END {if (NR) printf "%d passed, %d failed", p, f}')
  printf '%-20s %s  %4ss  %s\n' "$name" "$status" "$(( $(date +%s) - start ))" "$summary"
done

# Cargo rewrites workspace versions in Cargo.lock on every build; never let that leak into the stack.
git checkout -- Cargo.lock 2>/dev/null

if ((${#failed[@]})); then echo "FAILED: ${failed[*]} (logs in $LOG_DIR)"; exit 1; fi
echo "ALL PASS"
