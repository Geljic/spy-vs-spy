#!/bin/bash
# Run the patched vicious vcs2json over a Spy vs. Spy PS2 data dir with strict chunk checking
# (every chunk must end exactly at its BB BB BB BB end offset, file must end at EF FE EF FE),
# and print a pass/fail table per extension.
#
# usage: check.sh [DATA_DIR] [VCS2JSON] [LOG_DIR]
#   DATA_DIR  default: work/x            (extracted DATA.PAK)
#   VCS2JSON  default: work/vicious-ps2-build/apps/json/build/exe/vcs2json  (from setup.sh)
#   LOG_DIR   default: work/vicious-ps2-check   (one .log per failing file + results.tsv)
# Extra env: JOBS (default 4), JSON_DIR (if set, keep a .json per file there).
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
P=$(cd "$HERE/../.." && pwd)
DATA=${1:-$P/work/x}
BIN=${2:-$P/work/vicious-ps2-build/apps/json/build/exe/vcs2json}
LOGS=${3:-$P/work/vicious-ps2-check}
JOBS=${JOBS:-4}
[ -x "$BIN" ] || { echo "vcs2json not found: $BIN (run tools/vicious-ps2/setup.sh)"; exit 1; }
mkdir -p "$LOGS"
export BIN LOGS JSON_DIR=${JSON_DIR:-}

one() {
  f=$1; b=$(basename "$f"); ext=${b##*.}; rel=${f#"$DATA"/}
  out=/dev/null; [ -n "$JSON_DIR" ] && { mkdir -p "$JSON_DIR"; out="$JSON_DIR/${rel//\//_}.json"; }
  log="$LOGS/${rel//\//_}.log"
  timeout 300 nice -n 19 "$BIN" --strict --ps2 "$f" "$out" >/dev/null 2>"$log"; rc=$?
  if [ $rc -eq 0 ]; then st=pass; rm -f "$log"
  elif [ $rc -eq 2 ]; then st=fail
  elif [ $rc -eq 124 ]; then st=timeout
  elif [ $rc -eq 3 ]; then st=crash
  else st=crash; fi
  why=$(grep -m1 -E '^(ERROR|CRASH)' "$log" 2>/dev/null | cut -c1-120)
  printf '%s\t%s\t%s\t%s\n' "$ext" "$rel" "$st" "$why"
}
export -f one; export DATA

find "$DATA" -type f \( -name '*.gam' -o -name '*.map' -o -name '*.ls' -o -name '*.cin' -o -name '*.loc' \) | sort \
  | xargs -P "$JOBS" -I{} bash -c 'one "$1"' _ {} 2>/dev/null | sort > "$LOGS/results.tsv"

printf '%-5s %6s %6s %6s %6s %8s\n' ext files pass fail crash timeout
awk -F'\t' '{n[$1]++; c[$1" "$3]++} END {for (e in n) printf "%-5s %6d %6d %6d %6d %8d\n", e, n[e], c[e" pass"], c[e" fail"], c[e" crash"], c[e" timeout"]}' "$LOGS/results.tsv" | sort
awk -F'\t' '{n++; if ($3=="pass") p++} END {printf "total %d files, %d pass\n", n, p}' "$LOGS/results.tsv"
echo "details: $LOGS/results.tsv (failing files keep a .log with the chunk stack)"
