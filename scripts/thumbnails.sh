#!/usr/bin/env bash
# Regenerate the lobby thumbnails in assets/thumbnails/ by running each
# built-in experience and capturing its scene (needs a display and a GPU).
#
#   scripts/thumbnails.sh               # every built-in experience
#   scripts/thumbnails.sh iris-svm      # just one
#
# MNIST needs the data: it is downloaded once, or set BEVARU_MNIST_DIR.
# BEVARU_SCREENSHOT_AFTER sets how long each experience runs first (default 8 s).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release --features mnist
ids=("$@")
if [ ${#ids[@]} -eq 0 ]; then
    mapfile -t ids < <(./target/release/bevaru --list | awk 'NR > 1 { print $1 }')
fi
for id in "${ids[@]}"; do
    echo "== $id"
    ./target/release/bevaru "$id" --thumbnail "assets/thumbnails/$id.png"
done
