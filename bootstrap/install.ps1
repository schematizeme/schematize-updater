# Bootstrap do schematize-updater (Windows) — baixa o binário do updater e roda `install`.
# É a única coisa que o usuário roda à mão. Uso (PowerShell):
#   irm https://raw.githubusercontent.com/schematizeme/schematize-updater/main/bootstrap/install.ps1 | iex
$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo  = "schematizeme/schematize-updater"
$asset = "schematize-updater-windows-x86_64.exe"
$dest  = Join-Path $env:LOCALAPPDATA "schematize"
$bin   = Join-Path $dest "schematize-updater.exe"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

$url = "https://github.com/$repo/releases/latest/download/$asset"
Write-Host "-> baixando o schematize-updater ($asset)..."
try {
  Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $bin
} catch {
  Write-Host "nao achei o binario pre-compilado do updater ($url)."
  Write-Host "Se voce tem Rust: cargo install --git https://github.com/$repo"
  exit 1
}

# Garante o dir no PATH do usuario (perene).
$userPath = [Environment]::GetEnvironmentVariable('Path','User')
if ($userPath -notlike "*$dest*") {
  [Environment]::SetEnvironmentVariable('Path', "$userPath;$dest", 'User')
}

Write-Host "-> instalando o app schematize (binario pronto se houver, senao compila do fonte)..."
& $bin install
