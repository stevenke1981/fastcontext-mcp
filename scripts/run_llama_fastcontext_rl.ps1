<#
.SYNOPSIS
    Deploy FastContext-1.0-4B-RL via llama.cpp's llama-server (GGUF format).
.PARAMETER CtxSize
    Context window size. Default 262144 (max for FastContext).
    Reduce to 65536 or 32768 if memory constrained.
.PARAMETER Port
    Server port. Default 30000.
.PARAMETER Host
    Bind address. Default 0.0.0.0.
.PARAMETER NGpuLayers
    GPU layers (-1 = CPU only, 99 = max offload). Default 99.
.PARAMETER HfRepo
    HuggingFace GGUF repo. Default mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF.
.PARAMETER HfFile
    GGUF file in repo. Default fastcontext-1.0-4b-rl-q4_k_m.gguf.
#>

param(
    [int]    $CtxSize    = 262144,
    [int]    $Port       = 30000,
    [string] $Host       = "0.0.0.0",
    [int]    $NGpuLayers = 99,
    [string] $HfRepo     = "mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF",
    [string] $HfFile     = "fastcontext-1.0-4b-rl-q4_k_m.gguf"
)

# --- Pre-flight checks ---

# 1. Check llama-server is on PATH
$llamaServer = Get-Command "llama-server" -ErrorAction SilentlyContinue
if (-not $llamaServer) {
    Write-Error "llama-server not found. Install llama.cpp first:"
    Write-Error "  Windows: build from https://github.com/ggml-org/llama.cpp"
    Write-Error "  Or download release binaries from GitHub releases"
    exit 1
}

# 2. Estimate memory needed
# Q4_K_M ~2.5 GB model + KV cache ~2 bytes * n_layers * n_heads * ctx
# Rough: 2.5 GB + (CtxSize / 1024) * 0.5 GB
$estGb = [math]::Round(2.5 + ($CtxSize / 1024) * 0.5, 1)
Write-Host "[*] Estimated memory need: ~${estGb} GB (model 2.5 GB + KV cache)"
if ($estGb -gt 16) {
    Write-Warning "Context $CtxSize may need >16 GB RAM/VRAM. Consider --CtxSize 65536"
}

# --- Build command ---
$argsList = @(
    "--hf-repo", $HfRepo,
    "--hf-file", $HfFile,
    "--host", $Host,
    "--port", $Port.ToString(),
    "--ctx-size", $CtxSize.ToString(),
    "--jinja",
    "--flash-attn", "on"
)

if ($NGpuLayers -ge 0) {
    $argsList += "--n-gpu-layers"
    $argsList += $NGpuLayers.ToString()
}

Write-Host "[*] Starting llama-server..."
Write-Host "    Model : $HfRepo / $HfFile"
Write-Host "    Endpoint : http://${Host}:${Port}/v1/chat/completions"
Write-Host "    Context : $CtxSize tokens"
Write-Host ""

& "llama-server" $argsList
