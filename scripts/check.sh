#!/usr/bin/env bash
# The project's checks, run locally (there is no hosted CI).
#
#   scripts/check.sh          # does it compile? formatting + clippy on every
#                             # target (including tests) for each feature set
#   scripts/check.sh --tests  # also run the test suites (training, sweeps,
#                             # headless app simulations); slower
#
# Exits non-zero on the first failure.
set -euo pipefail
cd "$(dirname "$0")/.."

run_tests=false
[ "${1:-}" = "--tests" ] && run_tests=true

step() { printf '\n== %s\n' "$*"; }
start=$(date +%s)

step "format"
cargo fmt --all --check

# Clippy compiles every target (libraries, binaries, examples, tests)
# without running anything.
step "compile + clippy (default features)"
cargo clippy --workspace --all-targets -- -D warnings

step "compile + clippy (bevaru without default features: no math)"
cargo clippy -p bevaru --all-targets --no-default-features -- -D warnings

step "compile + clippy (mnist, remote)"
cargo clippy --workspace --all-targets --features mnist,remote -- -D warnings

if $run_tests; then
    step "tests"
    cargo test --workspace
    cargo test --workspace --features remote
    cargo test -p bevaru-core --features mnist

    step "bevaru-core has no Bevy dependency"
    if cargo tree -p bevaru-core --features mnist -e normal | grep -q '\bbevy'; then
        echo "bevaru-core must not depend on bevy" >&2
        exit 1
    fi

    step "no HTTP client without the mnist feature"
    if cargo tree -p bevaru -e normal | grep -q '\bureq\b'; then
        echo "ureq must only be pulled in by the mnist feature" >&2
        exit 1
    fi
fi

printf '\nAll checks passed in %ss.\n' "$(( $(date +%s) - start ))"
