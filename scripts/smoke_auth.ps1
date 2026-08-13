# Smoke test: register → character → play → print WS AuthHello JSON
# Requires: game-server running with migrated Postgres.
param(
    [string]$Base = "http://127.0.0.1:8080"
)

$ErrorActionPreference = "Stop"
$email = "pilot_$([guid]::NewGuid().ToString('N').Substring(0,8))@example.com"
$password = "password123"
$name = "Pilot$([guid]::NewGuid().ToString('N').Substring(0,6))"

Write-Host "Register $email"
$reg = Invoke-RestMethod -Method POST -Uri "$Base/auth/register" -ContentType "application/json" -Body (@{
    email = $email
    password = $password
} | ConvertTo-Json)

$headers = @{ Authorization = "Bearer $($reg.access_token)" }
Write-Host "Create character $name"
$ch = Invoke-RestMethod -Method POST -Uri "$Base/characters" -Headers $headers -ContentType "application/json" -Body (@{
    name = $name
} | ConvertTo-Json)

Write-Host "Play"
$play = Invoke-RestMethod -Method POST -Uri "$Base/auth/play" -ContentType "application/json" -Body (@{
    session_id = $reg.session_id
    refresh_token = $reg.refresh_token
    character_id = $ch.id
} | ConvertTo-Json)

$hello = @{
    t = "AuthHello"
    v = 1
    session_id = $play.session_id
    connect_ticket = $play.connect_ticket
    client_content_version = $play.content_version
    client_protocol_v = 1
} | ConvertTo-Json -Compress

Write-Host "AuthHello for WS $($Base -replace 'http','ws')/ws :"
Write-Host $hello
Write-Host "character=$($play.character_id) ship=$($play.ship_id) system=$($play.system_id)"
