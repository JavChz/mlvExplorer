<#
.SYNOPSIS
    Packages MLV Explorer for Windows x64 release (Portable Zip + Inno Setup Installer Exe).
.DESCRIPTION
    Builds the release binary, creates a staging directory with the binary,
    README.md, and LICENSE, compresses it into a portable zip archive, and
    builds the Windows Setup installer if Inno Setup 6 (ISCC.exe) is installed.
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
Write-Host "`n[1/4] Compiling release binary (target: $Target)..." -ForegroundColor Yellow
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
$PackageName = "mlv-explorer-v$Version-windows-x64-portable"
$StagingDir = "$DistDir\$PackageName"
$ZipFile = "$DistDir\$PackageName.zip"

Write-Host "`n[2/4] Staging portable files into $StagingDir..." -ForegroundColor Yellow
if (Test-Path $StagingDir) { Remove-Item $StagingDir -Recurse -Force }
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

Copy-Item $BinarySource -Destination "$StagingDir\mlv-explorer.exe"
Copy-Item "README.md" -Destination "$StagingDir\README.md"
Copy-Item "LICENSE" -Destination "$StagingDir\LICENSE"

# Compress into zip
Write-Host "`n[3/4] Creating portable archive: $ZipFile..." -ForegroundColor Yellow
if (Test-Path $ZipFile) { Remove-Item $ZipFile -Force }
Compress-Archive -Path "$StagingDir\*" -DestinationPath $ZipFile -CompressionLevel Optimal

# Cleanup staging folder
Remove-Item $StagingDir -Recurse -Force

$ZipSize = (Get-Item $ZipFile).Length / 1MB

# Build Windows Installer via Inno Setup 6 (if available)
Write-Host "`n[4/4] Building Windows Setup Installer (Inno Setup 6)..." -ForegroundColor Yellow
$SetupBase = "mlv-explorer-v$Version-windows-x64-setup"
$SetupExe = "$DistDir\$SetupBase.exe"
$IssPath = "$ProjectRoot\installer\mlv-explorer.iss"

$Iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source
if (-not $Iscc) {
    $Candidates = @(
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )
    foreach ($c in $Candidates) {
        if (Test-Path $c) {
            $Iscc = $c
            break
        }
    }
}

$BuiltInstaller = $false
if ($Iscc -and (Test-Path $IssPath)) {
    if (Test-Path $SetupExe) { Remove-Item $SetupExe -Force }
    & $Iscc "/DMyAppVersion=$Version" "/DSourceExe=$BinarySource" "/DOutputDir=dist" "/DOutputBaseFilename=$SetupBase" "$IssPath"
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Inno Setup compilation failed"
        exit $LASTEXITCODE
    }
    if (Test-Path $SetupExe) {
        $BuiltInstaller = $true
    }
} else {
    Write-Host "Notice: Inno Setup 6 (ISCC.exe) not found locally. Skipping local installer build." -ForegroundColor DarkYellow
    Write-Host "  Portable zip was still created, and GitHub Actions will build the installer on push." -ForegroundColor DarkGray
    Write-Host "  To build the installer locally, install Inno Setup: winget install JRSoftware.InnoSetup" -ForegroundColor DarkGray
}

Write-Host "`n==========================================" -ForegroundColor Green
Write-Host " Packaging completed successfully!" -ForegroundColor Green
Write-Host " Portable : $ZipFile ($([math]::Round($ZipSize, 2)) MB)" -ForegroundColor Green
if ($BuiltInstaller) {
    $ExeSize = (Get-Item $SetupExe).Length / 1MB
    Write-Host " Installer: $SetupExe ($([math]::Round($ExeSize, 2)) MB)" -ForegroundColor Green
}
Write-Host "==========================================" -ForegroundColor Green
Write-Host "`nOr run release.bat from repo root to bump Cargo.toml, tag v$Version, and trigger the GitHub Actions release pipeline.`n" -ForegroundColor Cyan
