# Start local development dependencies for Nova Horizon (Windows).
# Requires Docker Desktop (or compatible compose engine).

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$envFile = Join-Path $Root ".env"
$example = Join-Path $Root ".env.example"
if (-not (Test-Path $envFile)) {
    Write-Host "Creating .env from .env.example"
    Copy-Item $example $envFile
}

Write-Host "Starting Postgres..."
docker compose up -d postgres

Write-Host "Waiting for Postgres health..."
$deadline = (Get-Date).AddSeconds(60)
do {
    $status = docker compose ps --format json postgres 2>$null
    Start-Sleep -Seconds 1
    $healthy = docker inspect --format='{{.State.Health.Status}}' nova-horizon-postgres 2>$null
    if ($healthy -eq "healthy") { break }
} while ((Get-Date) -lt $deadline)

if ($healthy -ne "healthy") {
    Write-Warning "Postgres may still be starting. Check: docker compose ps"
} else {
    Write-Host "Postgres is healthy."
}

Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. cargo run -p game-server -- migrate   # after PR-03"
Write-Host "  2. cargo run -p game-server"
Write-Host "  3. Open client/ in Godot 4 and run the main scene"
Write-Host ""
Write-Host "Optional Redis: docker compose --profile redis up -d"
