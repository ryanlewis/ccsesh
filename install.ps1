#!/usr/bin/env pwsh
# install.ps1 - Install ccsesh on Windows
# Usage: irm https://raw.githubusercontent.com/ryanlewis/ccsesh/main/install.ps1 | iex

$ErrorActionPreference = 'Stop'

$Repo = "ryanlewis/ccsesh"
$Binary = "ccsesh"

$InstallDir = if ($env:CCSESH_INSTALL_DIR) {
    $env:CCSESH_INSTALL_DIR
} else {
    Join-Path $HOME ".ccsesh\bin"
}

# --- Helper functions ---
function Write-Info  { Write-Host "  info: " -ForegroundColor Green -NoNewline; Write-Host $args[0] }
function Write-Warn  { Write-Host "  warn: " -ForegroundColor Yellow -NoNewline; Write-Host $args[0] }
function Write-Err   { Write-Host "  error: " -ForegroundColor Red -NoNewline; Write-Host $args[0]; exit 1 }

# --- Architecture detection ---
if (-not [Environment]::Is64BitProcess) {
    Write-Err "64-bit Windows is required"
}

if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    Write-Err "ARM64 Windows is not yet supported"
}

$Target = "x86_64-pc-windows-msvc"
Write-Info "detected platform $Target"

# --- Version resolution ---
if ($env:VERSION) {
    $Version = $env:VERSION
    if (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }
} else {
    try {
        $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
        $Version = $Release.tag_name
    } catch {
        Write-Err "could not determine latest version (check network connectivity or set VERSION env var)"
    }
}

if (-not $Version) {
    Write-Err "could not determine latest version"
}

Write-Info "installing ccsesh $Version"

# --- Temp directory ---
$TmpFile = New-TemporaryFile
Remove-Item $TmpFile
$TmpDir = New-Item -ItemType Directory -Path $TmpFile.FullName
$Archive = "$Binary-$Target.zip"
$ArchivePath = Join-Path $TmpDir $Archive

try {
    # --- Download archive ---
    $Url = "https://github.com/$Repo/releases/download/$Version/$Archive"
    Write-Info "downloading from github.com/$Repo"

    try {
        Invoke-WebRequest -Uri $Url -OutFile $ArchivePath -UseBasicParsing
    } catch {
        Write-Err "download failed -- check that release $Version exists at github.com/$Repo/releases"
    }

    # --- Checksum verification ---
    if ($env:CCSESH_SKIP_CHECKSUM -eq "1") {
        Write-Warn "checksum verification skipped (CCSESH_SKIP_CHECKSUM=1)"
    } else {
        $ChecksumUrl = "https://github.com/$Repo/releases/download/$Version/$Archive.sha256"
        try {
            $ChecksumResponse = Invoke-WebRequest -Uri $ChecksumUrl -UseBasicParsing
            $Expected = ($ChecksumResponse.Content.Trim() -split '\s+')[0].ToLower()
            $Actual = (Get-FileHash -Path $ArchivePath -Algorithm SHA256).Hash.ToLower()

            if ($Actual -ne $Expected) {
                Write-Err "checksum mismatch (expected $Expected, got $Actual)"
            }
            Write-Info "checksum verified"
        } catch {
            if ($_.Exception.Message -match "checksum mismatch") {
                throw
            }
            Write-Err "could not verify checksum -- set CCSESH_SKIP_CHECKSUM=1 to bypass verification"
        }
    }

    # --- Extract ---
    $ExtractDir = Join-Path $TmpDir "extracted"
    Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

    # --- Install ---
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $SourceExe = Join-Path $ExtractDir "$Binary.exe"
    if (-not (Test-Path $SourceExe)) {
        # Binary might be at the root of the zip without a subdirectory
        $candidates = Get-ChildItem -Path $ExtractDir -Filter "$Binary.exe" -Recurse
        if ($candidates.Count -eq 0) {
            Write-Err "archive does not contain $Binary.exe"
        }
        $SourceExe = $candidates[0].FullName
    }

    $DestExe = Join-Path $InstallDir "$Binary.exe"
    Move-Item -Path $SourceExe -Destination $DestExe -Force

    Write-Info "installed to $DestExe"

    # --- PATH management ---
    $UserPath = [System.Environment]::GetEnvironmentVariable('Path', [System.EnvironmentVariableTarget]::User)
    if (-not (";$UserPath;".ToLower() -like "*;$($InstallDir.ToLower());*")) {
        if ($UserPath) {
            $NewPath = "$UserPath;$InstallDir"
        } else {
            $NewPath = $InstallDir
        }
        [System.Environment]::SetEnvironmentVariable(
            'Path',
            $NewPath,
            [System.EnvironmentVariableTarget]::User
        )
        Write-Warn "$InstallDir was not in your PATH -- it has been added (restart your terminal to pick it up)"
    }

    Write-Info "run 'ccsesh' to get started"
} finally {
    # --- Cleanup ---
    if (Test-Path $TmpDir) {
        Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
    }
}
