#!/usr/bin/env bash
# Record every built-in experience to video for a highlight reel (needs a
# display, a GPU, and ffmpeg). Each clip is 1920 × 1080, frame-locked, so it
# plays back smoothly at true speed however long the capture took.
#
#   scripts/record-demos.sh                  # every experience -> recordings/
#   scripts/record-demos.sh iris-svm sigmoid # just these
#
# Environment:
#   BEVARU_RECORD_DIR    output directory (default: recordings/)
#   BEVARU_RECORD_FPS    frame rate (default 30)
#   BEVARU_RECORD_CODEC  prores (default, .mov), h264 (.mp4), or vp9 (.webm)
#   BEVARU_PREVIEWS=1    also write a small H.264 preview of each clip
#
# The masters are ProRes 422 HQ: large (~100 MB per 10 s), but every frame
# is a keyframe, so editors cut and scrub them frame-accurately. Free
# DaVinci Resolve on Linux cannot decode H.264, which is another reason not
# to master in it.
#
# MNIST needs the data: it is downloaded once, or set BEVARU_MNIST_DIR.
set -euo pipefail
cd "$(dirname "$0")/.."

out="${BEVARU_RECORD_DIR:-recordings}"
codec="${BEVARU_RECORD_CODEC:-prores}"
case "$codec" in
    prores) ext=mov ;;
    h264) ext=mp4 ;;
    vp9) ext=webm ;;
    *) echo "unknown BEVARU_RECORD_CODEC: $codec" >&2; exit 2 ;;
esac
command -v ffmpeg >/dev/null || { echo "ffmpeg is required" >&2; exit 1; }

# id  seconds  orbit (deg/s; 3-D scenes turn so they read as 3-D)
plan=(
    "iris-svm               24  0"   # two passes of the C sweep
    "mnist-svm              15  0"   # settles by ~5 s
    "regression-mse-vs-mae  12  0"   # converges by ~5 s; hold the result
    "loss-curves            10  0"   # converges by ~5 s
    "loss-shapes            24  15"  # one full turn
    "loss-surface           24  15"
    "perceptron             20  9"
    "sigmoid                 8  0"   # a still plot: a short hold
)

cargo build --release --features mnist
mkdir -p "$out"

want=("$@")
failed=()
for row in "${plan[@]}"; do
    read -r id secs orbit <<<"$row"
    if [ ${#want[@]} -gt 0 ] && [[ ! " ${want[*]} " =~ " $id " ]]; then
        continue
    fi
    file="$out/$id.$ext"
    echo "== $id -> $file (${secs}s, orbit ${orbit}°/s)"
    if BEVARU_RECORD_SECS="$secs" BEVARU_RECORD_ORBIT="$orbit" BEVARU_RECORD_CODEC="$codec" \
        ./target/release/bevaru "$id" --record "$file"; then
        if [ "${BEVARU_PREVIEWS:-0}" = 1 ]; then
            ffmpeg -hide_banner -loglevel error -y -i "$file" \
                -vf scale=1280:-2 -c:v libopenh264 -b:v 4M -pix_fmt yuv420p \
                -movflags +faststart "$out/$id.preview.mp4"
        fi
    else
        failed+=("$id")
    fi
done

# Every requested id must be in the plan, so a new experience is not
# silently skipped.
for id in "${want[@]}"; do
    printf '%s\n' "${plan[@]}" | awk '{print $1}' | grep -qx "$id" \
        || { echo "not in the plan: $id (add it to scripts/record-demos.sh)" >&2; failed+=("$id"); }
done
if [ ${#want[@]} -eq 0 ]; then
    while read -r id; do
        printf '%s\n' "${plan[@]}" | awk '{print $1}' | grep -qx "$id" \
            || echo "warning: experience $id has no row in the plan and was not recorded" >&2
    done < <(./target/release/bevaru --list | awk 'NR > 1 { print $1 }')
fi

if [ ${#failed[@]} -gt 0 ]; then
    echo "failed: ${failed[*]}" >&2
    exit 1
fi
echo "done: $out/"
