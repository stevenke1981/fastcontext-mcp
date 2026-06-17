<#
.SYNOPSIS
    Deploy FastContext-1.0-4B-RL via llama.cpp's llama-server (GGUF format).
.PARAMETER CtxSize
    Context window size. Default 262144 (max for FastContext).
    Reduce to 65536 or 32768 if memory constrained.
.PARAMETER Port
    Server port. Default 30000.
.PARAMETER BindHost
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
    [string] $BindHost   = "0.0.0.0",
    [int]    $NGpuLayers = 99,
    [string] $HfRepo     = "mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF",
    [string] $HfFile     = "fastcontext-1.0-4b-rl-q4_k_m.gguf",
    [string] $LlamaDir   = "$env:USERPROFILE\.config\llama-cpp"
)

# --- Pre-flight checks ---

# 1. Locate llama-server (PATH first, then default install dir)
$llamaServer = Get-Command "llama-server" -ErrorAction SilentlyContinue
if (-not $llamaServer) {
    $fallback = "$LlamaDir\llama-server.exe"
    if (Test-Path -LiteralPath $fallback) {
        $llamaServer = $fallback
        Write-Host "[*] Found llama-server at: $fallback"
    } else {
        Write-Error "llama-server not found. Install llama.cpp:"
        Write-Error "  Download from: https://github.com/ggml-org/llama.cpp/releases"
        Write-Error "  Or copy to: $LlamaDir"
        exit 1
    }
} else {
    $llamaServer = $llamaServer.Source
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
    "--host", $BindHost,
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
Write-Host "    Endpoint : http://${BindHost}:${Port}/v1/chat/completions"
Write-Host "    Context : $CtxSize tokens"
Write-Host ""

& $llamaServer $argsList
