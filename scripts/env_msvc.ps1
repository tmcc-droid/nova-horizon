# Import MSVC linker environment for cargo on Windows (Developer PowerShell alternative).
# Usage (from repo root):
#   . .\scripts\env_msvc.ps1
#   cargo test --workspace

$ErrorActionPreference = "Stop"

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "vswhere not found. Install Visual Studio Build Tools 2022 with C++ workload."
}

$installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $installPath) {
    throw "MSVC VC tools not found. Install workload 'Desktop development with C++' / VCTools."
}

$vcvars = Join-Path $installPath "VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) {
    throw "vcvars64.bat missing at $vcvars"
}

# Capture environment from vcvars64.bat
$cmd = "`"$vcvars`" >nul && set"
cmd /c $cmd | ForEach-Object {
    if ($_ -match "^(.*?)=(.*)$") {
        $name = $matches[1]
        $value = $matches[2]
        [System.Environment]::SetEnvironmentVariable($name, $value, "Process")
    }
}

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) {
    $env:Path = "$cargoBin;$env:Path"
}

Write-Host "MSVC environment loaded from: $installPath"
Write-Host "cargo: $((Get-Command cargo -ErrorAction SilentlyContinue).Source)"
