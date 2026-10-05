#!/usr/bin/env bash
# Checks the standalone Jackdaw project in jackdaw/ (see docs/jackdaw.md).
# Opt-in and separate from scripts/check.sh: it fetches Jackdaw from git and
# compiles avian and the Jackdaw runtime, which takes minutes.
#
#   scripts/check-jackdaw.sh
#
# Run it after changing anything under jackdaw/ or bevaru's authorable
# components (src/authoring.rs). Exits non-zero on the first failure.
set -euo pipefail
cd "$(dirname "$0")/../jackdaw"

step() { printf '\n== %s\n' "$*"; }
start=$(date +%s)

step "format"
cargo fmt --check

step "compile + clippy"
cargo clippy --all-targets -- -D warnings

step "tests (scenes are current and load)"
cargo test

printf '\nJackdaw checks passed in %ss.\n' "$(( $(date +%s) - start ))"
