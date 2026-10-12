<#
.SYNOPSIS
    Packages MLV Explorer for Windows x64 release.
.DESCRIPTION
    Builds the release binary, creates a staging directory with the binary,
    README.md, and LICENSE, and compresses it into a distribution zip archive.
#>

[CmdletBinding()]
param(
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

# Navigate to project root
$ProjectRoot = Resolve-Path "$PSScriptRoot\.."
Set-Location $ProjectRoot

# Read version from Cargo.toml
$CargoToml = Get-Content "Cargo.toml" -Raw
if ($CargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Write-Error "Could not determine version from Cargo.toml"
    exit 1
}

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " Packaging MLV Explorer v$Version (Windows x64)" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# Build release executable
Write-Host "`n[1/3] Compiling release binary (target: $Target)..." -ForegroundColor Yellow
cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed"
    exit $LASTEXITCODE
}

$BinarySource = "target\$Target\release\mlv-explorer.exe"
if (-not (Test-Path $BinarySource)) {
    # Fallback to default target/release if standard target folder was not used
    $BinarySource = "target\release\mlv-explorer.exe"
}

if (-not (Test-Path $BinarySource)) {
    Write-Error "Compiled executable not found at $BinarySource"
    exit 1
}

# Prepare dist directory
$DistDir = "$ProjectRoot\dist"
$PackageName = "mlv-explorer-v$Version-windows-x64"
$StagingDir = "$DistDir\$PackageName"
$ZipFile = "$DistDir\$PackageName.zip"

Write-Host "`n[2/3] Staging files into $StagingDir..." -ForegroundColor Yellow
if (Test-Path $StagingDir) { Remove-Item $StagingDir -Recurse -Force }
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

Copy-Item $BinarySource -Destination "$StagingDir\mlv-explorer.exe"
Copy-Item "README.md" -Destination "$StagingDir\README.md"
Copy-Item "LICENSE" -Destination "$StagingDir\LICENSE"

# Compress into zip
Write-Host "`n[3/3] Creating distribution archive: $ZipFile..." -ForegroundColor Yellow
if (Test-Path $ZipFile) { Remove-Item $ZipFile -Force }
Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipFile -CompressionLevel Optimal

# Cleanup staging folder
Remove-Item $StagingDir -Recurse -Force

$ZipSize = (Get-Item $ZipFile).Length / 1MB
Write-Host "`n==========================================" -ForegroundColor Green
Write-Host " Package created successfully!" -ForegroundColor Green
Write-Host " File: $ZipFile" -ForegroundColor Green
Write-Host " Size: $([math]::Round($ZipSize, 2)) MB" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
Write-Host "`nTo publish this release to GitHub, run:" -ForegroundColor Cyan
Write-Host "  gh release create v$Version dist\$PackageName.zip --title `"v$Version`" --generate-notes`n" -ForegroundColor White
