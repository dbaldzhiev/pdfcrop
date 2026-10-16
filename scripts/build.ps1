# Builds the Windows installer locally.
#
#   pwsh scripts\build.ps1 -LocalGhostscript C:\ghostscript-10.08
#
# -LocalGhostscript points at an *unpacked* official Ghostscript Windows install
# (run gs10080w64.exe /S /D=C:\gs or extract it with 7z). If it is omitted, the
# script downloads the official installer, verifies its SHA-256 and unpacks it.
# CI does the same thing without arguments (.github/workflows/build.yml).

param(
    [string]$LocalGhostscript = "",
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $RepoRoot

$GsVersion = "10080"
$GsUrl = "https://github.com/ArtifexSoftware/ghostpdl-downloads/releases/download/gs$GsVersion/gs${GsVersion}w64.exe"
$GsSha256 = "52a91b8bf09298788d7a57b9206127026c23eacd75405f0a131e26dc381dce50"

try {
    if (-not $SkipTests) {
        Write-Host "=== Tests" -ForegroundColor Cyan
        cargo test --release
        if ($LASTEXITCODE -ne 0) { throw "tests failed" }
    }

    Write-Host "=== Building pdfcrop.exe" -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    if ($LocalGhostscript) {
        $gs = $LocalGhostscript
    } else {
        Write-Host "=== Fetching Ghostscript $GsVersion" -ForegroundColor Cyan
        if (-not (Test-Path gs-setup.exe)) {
            Invoke-WebRequest -Uri $GsUrl -OutFile gs-setup.exe
        }
        $hash = (Get-FileHash gs-setup.exe -Algorithm SHA256).Hash.ToLower()
        if ($hash -ne $GsSha256) { throw "Ghostscript hash mismatch: $hash" }
        New-Item -ItemType Directory -Force unpack | Out-Null
        Push-Location unpack
        7z x -y ..\gs-setup.exe | Out-Null
        Pop-Location
        if (-not (Test-Path unpack\bin\gswin64c.exe)) {
            Start-Process -Wait -FilePath .\gs-setup.exe -ArgumentList "/S", "/D=$PWD\unpack"
        }
        $gs = "unpack"
    }
    if (-not (Test-Path "$gs\bin\gswin64c.exe")) { throw "no gswin64c.exe under $gs" }

    Write-Host "=== Staging (tool + trimmed Ghostscript)" -ForegroundColor Cyan
    Remove-Item -Recurse -Force staging -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force staging\gs\bin, staging\gs\lib | Out-Null
    Copy-Item "$gs\bin\gswin64c.exe", "$gs\bin\gsdll64.dll" staging\gs\bin
    Copy-Item "$gs\lib" staging\gs\lib -Recurse
    Copy-Item "$gs\Resource" staging\gs\Resource -Recurse
    Copy-Item "$gs\iccprofiles" staging\gs\iccprofiles -Recurse
    if (Test-Path "$gs\doc\COPYING") { Copy-Item "$gs\doc\COPYING" staging\gs\COPYING }
    Copy-Item target\release\pdfcrop.exe staging\pdfcrop.exe
    $size = (Get-ChildItem staging\gs -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
    Write-Host ("  ghostscript payload: {0:N1} MB" -f $size)

    Write-Host "=== Inno Setup" -ForegroundColor Cyan
    $iscc = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $iscc) { throw "Inno Setup 6 not found: winget install JRSoftware.InnoSetup" }

    $version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
    Push-Location installer
    & $iscc "/DAppVersion=$version" pdfcrop.iss
    Pop-Location
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed" }

    Get-ChildItem dist
} finally {
    Pop-Location
}
