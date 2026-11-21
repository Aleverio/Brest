$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$sampleManifest = Join-Path $root 'figcraft-rust-export\dist\sample\figcraft_sample\Cargo.toml'
$samplePng = Join-Path $root 'figcraft-rust-export\dist\sample\verify.png'

function Invoke-Check {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )
    Write-Host "`n== $Label =="
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE"
    }
}

Push-Location $root
try {
    Invoke-Check 'workspace tests' @('test', '--workspace')
    Invoke-Check 'workspace clippy' @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Invoke-Check 'generate sample project' @('run', '--quiet', '-p', 'figcraft-rust-export', '--example', 'generate_sample')

    Invoke-Check 'generated: default features' @('test', '--manifest-path', $sampleManifest)
    Invoke-Check 'generated: cpu only' @('check', '--manifest-path', $sampleManifest, '--no-default-features', '--features', 'cpu')
    Invoke-Check 'generated: skia only' @('check', '--manifest-path', $sampleManifest, '--no-default-features', '--features', 'skia')
    Invoke-Check 'generated: gpu only' @('check', '--manifest-path', $sampleManifest, '--no-default-features', '--features', 'gpu')
    Invoke-Check 'generated: no window' @('check', '--manifest-path', $sampleManifest, '--no-default-features', '--features', 'cpu,skia,gpu')

    Invoke-Check 'runtime: cpu frames' @('run', '--quiet', '--manifest-path', $sampleManifest, '--', '--backend', 'cpu', '--frames', '3')
    Invoke-Check 'runtime: skia frames' @('run', '--quiet', '--manifest-path', $sampleManifest, '--', '--backend', 'skia', '--frames', '3')
    Invoke-Check 'runtime: gpu frames' @('run', '--quiet', '--manifest-path', $sampleManifest, '--', '--backend', 'gpu', '--frames', '3')
    Invoke-Check 'runtime: cpu png' @('run', '--quiet', '--manifest-path', $sampleManifest, '--', '--backend', 'cpu', '--png', $samplePng)

    if (-not (Test-Path $samplePng)) {
        throw "PNG was not written to $samplePng"
    }

    Write-Host "`n== plugin wasm + zip =="
    & (Join-Path $root 'scripts\build.ps1')
    if ($LASTEXITCODE -ne 0) {
        throw "plugin build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Write-Host "`nAll checks passed."
