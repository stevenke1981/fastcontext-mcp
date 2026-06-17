<#
.SYNOPSIS
    Remove fastcontext-mcp-rust installed files.
.DESCRIPTION
    Removes the binary from PATH location and the config directory.
.PARAMETER Prefix
    Install prefix. Default: $env:USERPROFILE\.cargo\bin
#>

param(
    [string] $Prefix = "$env:USERPROFILE\.config\fastcontext\bin"
)

$ErrorActionPreference = "Stop"
$ConfigDir = "$env:USERPROFILE\.config\fastcontext"

Write-Host "=== fastcontext-mcp-rust uninstaller ===`n" -ForegroundColor Cyan

$removed = $false

# Remove binary
$bin = "$Prefix\fastcontext-mcp-rust.exe"
if (Test-Path -LiteralPath $bin) {
    Remove-Item -LiteralPath $bin -Force
    Write-Host "  Removed: $bin"
    $removed = $true
} else {
    Write-Host "  Not found: $bin"
}

# Remove config directory
if (Test-Path -LiteralPath $ConfigDir) {
    Remove-Item -LiteralPath $ConfigDir -Recurse -Force
    Write-Host "  Removed: $ConfigDir"
    $removed = $true
} else {
    Write-Host "  Not found: $ConfigDir"
}

if ($removed) {
    Write-Host "`n=== Uninstall complete ===" -ForegroundColor Green
} else {
    Write-Host "`nNothing to remove." -ForegroundColor Yellow
}
