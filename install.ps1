<#
.SYNOPSIS
    Build and install fastcontext-mcp-rust system-wide.
.DESCRIPTION
    Builds the release binary, copies it and supporting files to
    $env:USERPROFILE\.cargo\bin\ and $env:USERPROFILE\.config\fastcontext-mcp\.
.PARAMETER NoBuild
    Skip cargo build --release; use existing binary from target/release/.
.PARAMETER Prefix
    Install prefix. Default: $env:USERPROFILE\.cargo\bin
#>

param(
    [switch] $NoBuild,
    [string] $Prefix = "$env:USERPROFILE\.config\fastcontext\bin"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSCommandPath
$ConfigDir = "$env:USERPROFILE\.config\fastcontext"

Write-Host "=== fastcontext-mcp-rust installer ===`n" -ForegroundColor Cyan

# ---- Step 1: Build ----
if (-not $NoBuild) {
    Write-Host "[1/4] Building release binary..." -ForegroundColor Yellow
    Push-Location $RepoRoot
    try {
        & "cargo" "build", "--release"
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "[1/4] Skipping build (--NoBuild)" -ForegroundColor Yellow
}

# ---- Step 2: Verify binary ----
Write-Host "[2/4] Verifying binary..." -ForegroundColor Yellow
$BinSource = "$RepoRoot\target\release\fastcontext-mcp-rust.exe"
if (-not (Test-Path -LiteralPath $BinSource)) {
    Write-Error "Binary not found: $BinSource. Run without --NoBuild or build manually."
    exit 1
}

# ---- Step 3: Install ----
Write-Host "[3/4] Installing..." -ForegroundColor Yellow

# Binary
if (-not (Test-Path -LiteralPath $Prefix)) {
    New-Item -ItemType Directory -Path $Prefix -Force | Out-Null
}
Copy-Item -LiteralPath $BinSource -Destination "$Prefix\fastcontext-mcp-rust.exe" -Force
Write-Host "  Binary -> $Prefix\fastcontext-mcp-rust.exe"

# Config & supporting files
$TargetScripts = "$ConfigDir\scripts"
$TargetExamples = "$ConfigDir\examples"
New-Item -ItemType Directory -Path $TargetScripts -Force | Out-Null
New-Item -ItemType Directory -Path $TargetExamples -Force | Out-Null

Copy-Item -LiteralPath "$RepoRoot\scripts\run_llama_fastcontext_rl.ps1" -Destination "$TargetScripts\" -Force
Copy-Item -LiteralPath "$RepoRoot\scripts\run_sglang_fastcontext_rl.ps1" -Destination "$TargetScripts\" -Force
Copy-Item -LiteralPath "$RepoRoot\examples\opencode.jsonc" -Destination "$TargetExamples\" -Force
Write-Host "  Scripts  -> $TargetScripts"
Write-Host "  Examples -> $TargetExamples"

# ---- Step 4: PATH check ----
Write-Host "[4/4] Checking PATH..." -ForegroundColor Yellow
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -and $userPath.Contains($Prefix)) {
    Write-Host "  $Prefix already on PATH" -ForegroundColor Green
} else {
    Write-Host "  WARNING: $Prefix is not on your PATH." -ForegroundColor Yellow
    Write-Host "  Add it manually or re-run your shell profile."
    Write-Host "  Suggested: [Environment]::SetEnvironmentVariable('Path',"
    Write-Host "    [Environment]::GetEnvironmentVariable('Path','User') + ';$Prefix', 'User')"
}

Write-Host "`n=== Install complete ===" -ForegroundColor Green
Write-Host "Run 'fastcontext-mcp-rust.exe' to start the MCP server."
Write-Host "Model scripts at: $TargetScripts"
Write-Host "Example config at: $TargetExamples"
