$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$wasm = Join-Path $root 'target\wasm32-unknown-unknown\release\figcraft_rust_export.wasm'
$pluginDir = Join-Path $root 'figcraft-rust-export'
$manifest = Join-Path $pluginDir 'figcraft-plugin.json'
$installedWasm = Join-Path $pluginDir 'figcraft_rust_export.wasm'
$dist = Join-Path $root 'dist'
$packageDir = Join-Path $dist 'figcraft-rust-export'
$zip = Join-Path $dist 'figcraft-rust-export-plugin.zip'

Write-Host 'Building the FigCraft plugin for wasm32-unknown-unknown...'
Push-Location $root
try {
    cargo build --release --target wasm32-unknown-unknown -p figcraft-rust-export
}
finally {
    Pop-Location
}

if (-not (Test-Path $wasm)) {
    throw "Build did not produce $wasm"
}

Copy-Item $wasm $installedWasm -Force

New-Item -ItemType Directory -Force -Path $dist | Out-Null
if (Test-Path $packageDir) {
    Remove-Item -Recurse -Force $packageDir
}
New-Item -ItemType Directory -Force -Path $packageDir | Out-Null
Copy-Item $manifest $packageDir -Force
Copy-Item $installedWasm $packageDir -Force
if (Test-Path $zip) {
    Remove-Item -Force $zip
}
Compress-Archive -Path (Join-Path $packageDir '*') -DestinationPath $zip -Force

Write-Host ''
Write-Host "Plugin folder : $pluginDir"
Write-Host "Import zip    : $zip"
Get-Item $installedWasm | Select-Object FullName, Length
