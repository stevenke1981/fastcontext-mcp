#!/usr/bin/env bash
#
# Remove fastcontext-mcp-rust installed files.
#
# Usage:
#   ./uninstall.sh
#   PREFIX=/usr/local/bin ./uninstall.sh
#
set -euo pipefail

PREFIX="${PREFIX:-"$HOME/.config/fastcontext/bin"}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/fastcontext"

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${CYAN}=== fastcontext-mcp-rust uninstaller ===${NC}\n"

REMOVED=false

# Remove binary
BIN="$PREFIX/fastcontext-mcp-rust"
if [ -f "$BIN" ]; then
    rm -f "$BIN"
    echo "  Removed: $BIN"
    REMOVED=true
else
    echo "  Not found: $BIN"
fi

# Remove config directory
if [ -d "$CONFIG_DIR" ]; then
    rm -rf "$CONFIG_DIR"
    echo "  Removed: $CONFIG_DIR"
    REMOVED=true
else
    echo "  Not found: $CONFIG_DIR"
fi

if [ "$REMOVED" = true ]; then
    echo -e "\n${GREEN}=== Uninstall complete ===${NC}"
else
    echo -e "\n${YELLOW}Nothing to remove.${NC}"
fi
