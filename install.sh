#!/usr/bin/env bash
#
# Build and install fastcontext-mcp-rust system-wide.
#
# Usage:
#   ./install.sh              # build + install to ~/.cargo/bin
#   ./install.sh --no-build   # skip build, use existing binary
#   PREFIX=/usr/local/bin ./install.sh
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd -P)"
PREFIX="${PREFIX:-"$HOME/.config/fastcontext/bin"}"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/fastcontext"

CYAN='\033[0;36m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== fastcontext-mcp-rust installer ===${NC}\n"

NO_BUILD="${1:-}"
if [ "$NO_BUILD" = "--no-build" ]; then
    echo -e "${YELLOW}[1/4] Skipping build (--no-build)${NC}"
else
    echo -e "${YELLOW}[1/4] Building release binary...${NC}"
    (cd "$REPO_ROOT" && cargo build --release)
fi

echo -e "${YELLOW}[2/4] Verifying binary...${NC}"
BIN_SOURCE="$REPO_ROOT/target/release/fastcontext-mcp-rust"
if [ ! -f "$BIN_SOURCE" ]; then
    echo "[ERROR] Binary not found: $BIN_SOURCE" >&2
    exit 1
fi

echo -e "${YELLOW}[3/4] Installing...${NC}"
mkdir -p "$PREFIX" "$CONFIG_DIR/scripts" "$CONFIG_DIR/examples"

cp "$BIN_SOURCE" "$PREFIX/fastcontext-mcp-rust"
chmod +x "$PREFIX/fastcontext-mcp-rust"
echo "  Binary -> $PREFIX/fastcontext-mcp-rust"

cp "$REPO_ROOT/scripts/run_llama_fastcontext_rl.sh" "$CONFIG_DIR/scripts/"
cp "$REPO_ROOT/scripts/run_sglang_fastcontext_rl.sh" "$CONFIG_DIR/scripts/"
cp "$REPO_ROOT/examples/opencode.jsonc" "$CONFIG_DIR/examples/"
echo "  Scripts  -> $CONFIG_DIR/scripts"
echo "  Examples -> $CONFIG_DIR/examples"

echo -e "${YELLOW}[4/4] Checking PATH...${NC}"
if [[ ":$PATH:" == *":$PREFIX:"* ]]; then
    echo -e "${GREEN}  $PREFIX already on PATH${NC}"
else
    echo -e "${YELLOW}  WARNING: $PREFIX is not on your PATH.${NC}"
    echo "  Add 'export PATH=\"\$PATH:$PREFIX\"' to your ~/.bashrc or ~/.zshrc"
fi

echo -e "\n${GREEN}=== Install complete ===${NC}"
echo "Run 'fastcontext-mcp-rust' to start the MCP server."
echo "Model scripts at: $CONFIG_DIR/scripts"
echo "Example config at: $CONFIG_DIR/examples"
