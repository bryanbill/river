#Requires -Version 5.0
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\river"
)

$ErrorActionPreference = "Stop"

$Repo = "bryanbill/river"

function Write-Bold  { Write-Host $args -ForegroundColor White }
function Write-Err   { Write-Host $args -ForegroundColor Red }
function Write-Info  { Write-Host $args -ForegroundColor Blue }

# ---- detect platform ----
function Get-PlatformInfo {
    $arch = (Get-CimInstance Win32_Processor).Architecture
    switch ($arch) {
        0   { $archStr = "x86_64" }    # x86
        5   { $archStr = "aarch64" }   # ARM
        9   { $archStr = "x86_64" }    # x64 (AMD64)
        12  { $archStr = "aarch64" }   # ARM64
        default {
            Write-Err "Unsupported architecture: $arch"
            exit 1
        }
    }

    $target = "${archStr}-pc-windows-msvc"
    return $target
}

# ---- download and install ----
function Install-River {
    $target = Get-PlatformInfo
    $filename = "river-${target}.zip"
    $releaseUrl = "https://github.com/${Repo}/releases/latest/download/${filename}"
    $binary = "river.exe"

    Write-Info "Detected  : ${target}"
    Write-Info "Downloading: ${releaseUrl}"

    $tmpdir = Join-Path $env:TEMP "river_install_$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpdir -Force | Out-Null

    try {
        $zipPath = Join-Path $tmpdir $filename

        if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
            & curl.exe -fsSL $releaseUrl -o $zipPath
            if ($LASTEXITCODE -ne 0) { throw "curl download failed" }
        } else {
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            Invoke-WebRequest -Uri $releaseUrl -OutFile $zipPath
        }

        Write-Info "Extracting..."
        Expand-Archive -Path $zipPath -DestinationPath $tmpdir -Force

        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        $dest = Join-Path $InstallDir $binary
        $src = Get-ChildItem -Path $tmpdir -Filter $binary -Recurse | Select-Object -First 1
        if (-not $src) {
            Write-Err "Could not find ${binary} in extracted archive"
            exit 1
        }

        Write-Info "Installing to: ${dest}"
        Copy-Item -Path $src.FullName -Destination $dest -Force

        # ---- add to PATH ----
        $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
        if ($userPath -notlike "*${InstallDir}*") {
            Write-Info "Adding ${InstallDir} to your user PATH"
            [Environment]::SetEnvironmentVariable(
                "Path",
                "${userPath};${InstallDir}",
                "User"
            )
            $env:Path = "${env:Path};${InstallDir}"
        }

        Write-Bold "River installed successfully!"
        Write-Info "Run:  river --help"
    }
    finally {
        Remove-Item -Recurse -Force $tmpdir -ErrorAction SilentlyContinue
    }
}

Install-River
