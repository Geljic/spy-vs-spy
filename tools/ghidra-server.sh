#!/usr/bin/env bash
# ghidra-mcp server on the PS2 ELF (work/ghidra_proj/svs.gpr). Args: none to start, or stop / status.
# Details: docs/reverse-engineering-workflow.md in the monorepo.
here="$(cd "$(dirname "$0")/.." && pwd)"
s="$here/../../shared/scripts/ghidra-server.sh"
case "${1:-}" in stop|status) exec "$s" "$1" ;; esac
# this project's own Ghidra has the PS2 (R5900) processor extension
export GHIDRA_HOME="$here/work/tools/ghidra_12.1.4_PUBLIC"
exec "$s" "$here/work/ghidra_proj/svs.gpr" /SLES_530.78
