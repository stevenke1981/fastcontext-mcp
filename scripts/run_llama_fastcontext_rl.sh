#!/usr/bin/env bash
#
# Deploy FastContext-1.0-4B-RL via llama.cpp's llama-server (GGUF format).
#
# Usage:
#   ./scripts/run_llama_fastcontext_rl.sh [--ctx-size 131072] [--port 30000]
#
# Options:
#   --ctx-size N    Context window (default: 262144, reduce to 65536 if OOM)
#   --port N        Server port (default: 30000)
#   --host ADDR     Bind address (default: 0.0.0.0)
#   --ngl N         GPU layers (-1 = CPU, 99 = max offload, default: 99)
#   --hf-repo REPO  HuggingFace GGUF repo (default: mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF)
#   --hf-file FILE  GGUF file in repo (default: fastcontext-1.0-4b-rl-q4_k_m.gguf)
#

set -euo pipefail

# ---- Defaults ----
CTX_SIZE=262144
PORT=30000
HOST="0.0.0.0"
NGPU_LAYERS=99
HF_REPO="mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF"
HF_FILE="fastcontext-1.0-4b-rl-q4_k_m.gguf"

# ---- Parse args ----
while [[ $# -gt 0 ]]; do
    case "$1" in
        --ctx-size) CTX_SIZE="$2"; shift 2 ;;
        --port)     PORT="$2"; shift 2 ;;
        --host)     HOST="$2"; shift 2 ;;
        --ngl)      NGPU_LAYERS="$2"; shift 2 ;;
        --hf-repo)  HF_REPO="$2"; shift 2 ;;
        --hf-file)  HF_FILE="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ---- Pre-flight checks ----
if ! command -v llama-server &>/dev/null; then
    echo "[ERROR] llama-server not found. Install llama.cpp first:"
    echo "  Linux/macOS: brew install llama.cpp"
    echo "  Or build from https://github.com/ggml-org/llama.cpp"
    exit 1
fi

# Rough memory estimate (Q4_K_M ~2.5 GB + KV cache)
EST_GB=$(echo "scale=1; 2.5 + ($CTX_SIZE / 1024) * 0.5" | bc -l 2>/dev/null || echo "?")
echo "[*] Estimated memory need: ~${EST_GB} GB (model 2.5 GB + KV cache)"
if [[ "$EST_GB" != "?" ]] && (( $(echo "$EST_GB > 16" | bc -l 2>/dev/null || echo 0) )); then
    echo "[WARNING] Context $CTX_SIZE may need >16 GB RAM/VRAM. Consider --ctx-size 65536"
fi

# ---- Build command ----
CMD="llama-server"
CMD+=" --hf-repo \"$HF_REPO\""
CMD+=" --hf-file \"$HF_FILE\""
CMD+=" --host \"$HOST\""
CMD+=" --port $PORT"
CMD+=" --ctx-size $CTX_SIZE"
CMD+=" --jinja"
CMD+=" --flash-attn on"

if [ "$NGPU_LAYERS" -ge 0 ] 2>/dev/null; then
    CMD+=" --n-gpu-layers $NGPU_LAYERS"
fi

echo "[*] Starting llama-server..."
echo "    Model    : $HF_REPO / $HF_FILE"
echo "    Endpoint : http://${HOST}:${PORT}/v1/chat/completions"
echo "    Context  : $CTX_SIZE tokens"
echo ""

eval "$CMD"
