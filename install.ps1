# install.ps1 — install the `lens` static-analysis CLI on Windows.
#
# Usage (run in PowerShell):
#   iwr -useb https://raw.githubusercontent.com/fatmuh/lens/main/install.ps1 | iex
#
# Environment variables (all optional):
#   $env:LENS_VERSION     — release tag, e.g. v0.1.0.  Default: latest.
#   $env:LENS_REPO        — GitHub owner/repo.         Default: fatmuh/lens.
#   $env:LENS_INSTALL_DIR — destination directory.     Default: $env:USERPROFILE\bin.

$ErrorActionPreference = 'Stop'

$Repo = if ($env:LENS_REPO) { $env:LENS_REPO } else { 'fatmuh/lens' }
$Version = if ($env:LENS_VERSION) { $env:LENS_VERSION } else { 'latest' }
$InstallDir = if ($env:LENS_INSTALL_DIR) { $env:LENS_INSTALL_DIR } else { "$env:USERPROFILE\bin" }

# --- Detect architecture ----------------------------------------------------
$Arch = $env:PROCESSOR_ARCHITECTURE
switch ($Arch) {
    'AMD64' { $Target = 'x86_64-pc-windows-msvc' }
    'ARM64' { $Target = 'aarch64-pc-windows-msvc' }
    default {
        Write-Error "Unsupported architecture: $Arch"
        exit 1
    }
}

# --- Build download URL -----------------------------------------------------
if ($Version -eq 'latest') {
    $Url = "https://github.com/$Repo/releases/latest/download/lens-$Target.zip"
} else {
    $Url = "https://github.com/$Repo/releases/download/$Version/lens-$Target.zip"
}

Write-Host "Installing lens $Version for $Target..." -ForegroundColor Cyan

# --- Download & extract ----------------------------------------------------
$Tmp = Join-Path $env:TEMP ("lens-install-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null

try {
    $ZipPath = Join-Path $Tmp 'lens.zip'
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
    Expand-Archive -Path $ZipPath -DestinationPath $Tmp -Force

    $ExePath = Join-Path $Tmp 'lens.exe'
    if (-not (Test-Path $ExePath)) {
        Write-Error "Downloaded archive did not contain lens.exe"
        exit 1
    }

    # --- Install ---------------------------------------------------------
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Move-Item -Path $ExePath -Destination (Join-Path $InstallDir 'lens.exe') -Force

    Write-Host ""
    Write-Host "✓ lens installed to $InstallDir\lens.exe" -ForegroundColor Green
    Write-Host ""
    & "$InstallDir\lens.exe" --version

    # --- Add to PATH if needed ------------------------------------------
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Adding $InstallDir to your PATH..." -ForegroundColor Yellow
        [Environment]::SetEnvironmentVariable('Path', "$UserPath;$InstallDir", 'User')
        $env:Path = "$env:Path;$InstallDir"
        Write-Host "Done. Restart your terminal for the change to take effect everywhere." -ForegroundColor Green
    }
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
