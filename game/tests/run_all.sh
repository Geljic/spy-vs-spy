#!/usr/bin/env bash
# Run every test script against the game build. Usage: tests/run_all.sh [release]
cd "$(dirname "$0")" || exit 1
profile=${1:-debug}
bin=../target/$profile/spy-vs-spy
[ -x "$bin" ] || { echo "build first: cargo build"; exit 1; }
export SVS_SHOT_DIR="$(cd ../.. && pwd)/work"; mkdir -p "$SVS_SHOT_DIR"
pass=0; fail=0
for t in *.script; do
  out=$(SVS_SCRIPT="$t" timeout 120 "$bin" 2>/dev/null); code=$?
  grep -E '^(  ok|  FAIL|PASS|FAIL)' <<<"$out"
  if [ "$code" = 0 ] && grep -q '^PASS' <<<"$out"; then pass=$((pass+1)); else fail=$((fail+1)); echo "   (exit code $code)"; fi
done
# real levels from the disc: spies must walk on every arena without falling out
# focus maps only (finish these fully before widening)
for m in Mansion_Story CreepyCarnival; do
  out=$(SVS_MAP=$m SVS_ARENA_TEST=1 timeout 120 "$bin" 2>/dev/null); code=$?
  grep -E '^(PASS|FAIL|  FAIL)' <<<"$out"
  if [ "$code" = 0 ] && grep -q '^PASS' <<<"$out"; then pass=$((pass+1)); else fail=$((fail+1)); fi
done
echo "== $pass passed, $fail failed"
[ "$fail" = 0 ]
