#!/usr/bin/env bash
# cargo wrapper for building the SIG Codex fork quickly and correctly on Windows.
#
#   scripts/cargo-sig.sh [fast|final] <cargo args...>
#
# fast  (default): release without LTO, 16 codegen units, rust-lld. For local
#                  installs and benchmarks while iterating; roughly 4-5x faster to build.
# final:           upstream's release profile (thin LTO, 4 codegen units), for the
#                  binary we ship or measure as "the" build.
#
# Both modes:
#   - use sccache, a content-addressed compile cache shared by every worktree, so a
#     new worktree reuses compiled dependencies. Unlike a shared target dir, it
#     can't serve stale outputs.
#   - hide Git for Windows' coreutils link.exe, which shadows the MSVC linker.
#   - restore Cargo.lock afterwards (cargo rewrites workspace versions).
set -uo pipefail

mode=fast
case "${1:-}" in fast|final) mode="$1"; shift ;; esac

PATH="$(echo "$PATH" | tr ':' '\n' | grep -v '/usr/bin$' | paste -sd:):/usr/bin"
export PATH
if command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER=sccache SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-60G}"
fi
if [ "$mode" = fast ]; then
  export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  export RUSTFLAGS="${RUSTFLAGS:-} -C linker=rust-lld"
fi

cargo "$@"
status=$?
git checkout -- Cargo.lock 2>/dev/null || true
exit $status
