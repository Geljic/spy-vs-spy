#!/bin/bash
# Record the Mission 2 (Kooky Carnival) scripted run as a short video for Tyson / devlog.
#   tools/m2-video.sh [repo dir] [out.mp4] [title subline]
# Renders every 6th sim step (P11_REC) → plays at 3× real time, 30 fps; adds a title card and an end card.
# Silent (the engine has no audio). Builds the binary first via run.sh m2 (also the PASS check).
set -euo pipefail
R=${1:-$(dirname "$0")/../work/friend-SpyVsSpy}; R=$(cd "$R" && pwd)
OUT=$(realpath -m "${2:-$(dirname "$0")/../dist/spy-vs-spy-m2-$(date +%F).mp4}")
SUB=${3:-"Kooky Carnival, scripted run at 3x"}
FONT=/usr/share/fonts/google-noto/NotoSans-Bold.ttf
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cd "$R"
[ "${SKIP_CHECK:-0}" = 1 ] || poc/11_gameplay/run.sh m2 | tail -1
mkfifo "$T/f"
ffmpeg -nostdin -loglevel error -y -f image2pipe -c:v ppm -framerate 30 -i "$T/f" -c:v libx264 -pix_fmt yuv420p -crf 20 "$T/run.mp4" > "$T/ff.log" 2>&1 &
FF=$!
exec 3>"$T/f"   # hold the FIFO open: the recorder opens+closes it per frame, ffmpeg would see EOF after frame 1
P11_REC="$T/f,6" work/poc11/bin mtest "$T/x.ppm" 692B67BD109F597F > "$T/run.log" 2>&1 &
BIN=$!
while kill -0 $BIN 2>/dev/null; do kill -0 $FF 2>/dev/null || { echo "ffmpeg died:"; cat "$T/ff.log"; kill $BIN; exit 1; }; sleep 2; done
exec 3>&-
wait $FF
grep -c '^  ok' "$T/run.log" | sed 's/^/checks ok: /'
read W H < <(ffprobe -v error -select_streams v -show_entries stream=width,height -of csv=p=0:s=' ' "$T/run.mp4")
card() { # card <out> <seconds> <big line> <small line>
  ffmpeg -nostdin -loglevel error -y -f lavfi -i "color=c=0x101418:s=${W}x${H}:d=$2:r=30" -vf \
    "drawtext=fontfile=$FONT:text='$3':fontcolor=white:fontsize=$((H/12)):x=(w-tw)/2:y=h/2-th,\
drawtext=fontfile=$FONT:text='$4':fontcolor=0xb0c4d0:fontsize=$((H/26)):x=(w-tw)/2:y=h/2+$((H/20)),\
fade=t=in:d=0.4,fade=t=out:st=$(echo "$2-0.4" | bc):d=0.4" -c:v libx264 -pix_fmt yuv420p "$1"; }
card "$T/a.mp4" 3 "Spy vs. Spy - Mission 2" "$SUB"
card "$T/b.mp4" 4 "$(git log -1 --format=%h) on $(git rev-parse --abbrev-ref HEAD)" "the player's own PS2 data, native PC engine"
printf "file '%s'\n" "$T/a.mp4" "$T/run.mp4" "$T/b.mp4" > "$T/list"
ffmpeg -nostdin -loglevel error -y -f concat -safe 0 -i "$T/list" -c copy "$OUT"
echo "wrote $OUT ($(ffprobe -v error -show_entries format=duration -of csv=p=0 "$OUT" | cut -d. -f1) s)"
