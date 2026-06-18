$ErrorActionPreference = "Stop"

$Root = (Resolve-Path "$PSScriptRoot\..").Path
$Bash = $env:MSYS2_BASH

if ([string]::IsNullOrWhiteSpace($Bash)) {
  $Candidates = @(
    "C:\msys64\usr\bin\bash.exe",
    "C:\msys64\mingw64\bin\bash.exe",
    "C:\msys64\clang64\bin\bash.exe"
  )
  foreach ($Candidate in $Candidates) {
    if (Test-Path $Candidate) {
      $Bash = $Candidate
      break
    }
  }
}

if ([string]::IsNullOrWhiteSpace($Bash) -or !(Test-Path $Bash)) {
  Write-Error "MSYS2 bash is required to build FFmpeg on Windows. Set MSYS2_BASH or install MSYS2."
}

$RootForBash = $Root -replace "\\", "/"
if ($RootForBash -match "^([A-Za-z]):/(.*)$") {
  $Drive = $Matches[1].ToLowerInvariant()
  $Rest = $Matches[2]
  $RootForBash = "/$Drive/$Rest"
}

& $Bash -lc "cd '$RootForBash' && scripts/build-ffmpeg.sh windows"
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
