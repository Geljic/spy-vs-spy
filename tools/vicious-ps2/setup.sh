#!/bin/bash
# The patches in this folder modify vicious (https://github.com/vicious-rebirth/vicious),
# Copyright (c) Alexandre Lavoie, MIT License. The patched files keep that license.
#
# Build vicious with our PS2 patches:
#   clones vicious at 1135220 into work/vicious-ps2-build, applies NNNN-*.patch,
#   runs the C codegen and a Release build of the CLI apps (nice'd, at most 4 jobs).
#
# usage: tools/vicious-ps2/setup.sh [BUILD_DIR]
# Needs: git, node 22 + pnpm, cmake, a C compiler, network access for the clone and cJSON/cglm.
# Afterwards: tools/vicious-ps2/check.sh  (strict decode of every container file)
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
P=$(cd "$HERE/../.." && pwd)
DIR=${1:-$P/work/vicious-ps2-build}
BASE=1135220
JOBS=${JOBS:-4}
APPS="json texture mesh animation audio pack material pak"

if [ ! -d "$DIR/.git" ]; then
    git clone https://github.com/vicious-rebirth/vicious.git "$DIR"
fi

cd "$DIR"
if git rev-parse -q --verify ps2 >/dev/null; then
    echo "branch ps2 already exists in $DIR, using it as is"
    git checkout -q ps2
else
    git checkout -q -b ps2 "$BASE"
    git -c user.name=vicious-ps2 -c user.email=vicious-ps2@localhost am -q "$HERE"/[0-9][0-9][0-9][0-9]-*.patch
fi

nice -n 19 pnpm install --ignore-scripts

# Codegen directly (turbo's cache can skip it after schema changes)
(cd packages/c && nice -n 19 pnpm codegen)

for app in $APPS; do
    echo "== apps/$app"
    (cd "apps/$app" && nice -n 19 cmake -DCMAKE_BUILD_TYPE=Release -S . -B build >/dev/null \
        && nice -n 19 cmake --build build -j"$JOBS")
done

echo
echo "Built:"
ls apps/*/build/exe/
echo
echo "PS2 data: pass --ps2 to vcs2json, or set VICIOUS_PLATFORM=ps2 for every tool."
