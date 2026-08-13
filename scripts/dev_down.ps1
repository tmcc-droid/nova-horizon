# Stop local development stack. Use -Wipe to destroy volumes (alpha wipe).
param(
    [switch]$Wipe
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if ($Wipe) {
    Write-Host "Stopping stack and wiping volumes..."
    docker compose --profile redis down -v
} else {
    Write-Host "Stopping stack (volumes kept)..."
    docker compose --profile redis down
}

Write-Host "Done."
