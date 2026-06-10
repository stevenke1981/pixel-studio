param(
    [switch]$Clean,
    [switch]$RunTests,
    [string]$DistDir = "dist"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    throw "This release build script is intended for Windows."
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$distPath = Join-Path $repoRoot $DistDir
$releasePath = Join-Path $repoRoot "target\release"

Push-Location $repoRoot
try {
    if ($Clean) {
        Write-Host "Cleaning release artifacts..."
        cargo clean
    }

    if ($RunTests) {
        Write-Host "Running workspace tests..."
        cargo test --workspace
    }

    Write-Host "Building release workspace..."
    cargo build --release --workspace

    New-Item -ItemType Directory -Force -Path $distPath | Out-Null

    $binaries = @(
        "studio-app.exe",
        "studio-cli.exe"
    )

    foreach ($binary in $binaries) {
        $source = Join-Path $releasePath $binary
        if (-not (Test-Path -LiteralPath $source)) {
            throw "Expected release binary was not found: $source"
        }

        Copy-Item -LiteralPath $source -Destination (Join-Path $distPath $binary) -Force
    }

    Write-Host ""
    Write-Host "Release build complete:"
    foreach ($binary in $binaries) {
        $output = Join-Path $distPath $binary
        $item = Get-Item -LiteralPath $output
        Write-Host ("  {0} ({1:N0} bytes)" -f $item.FullName, $item.Length)
    }
}
finally {
    Pop-Location
}
