$ErrorActionPreference = "Stop"
$FontFile = "assets\fonts\HarmonyOS_Sans_SC_Regular.ttf"
$FontLicenseFile = "assets\fonts\HarmonyOS_Sans_SC_LICENSE.txt"
$NoticeFile = "NOTICE.md"
$ThirdPartyFile = "THIRD_PARTY_NOTICES.md"

if (!(Test-Path $FontFile) -or !(Test-Path $FontLicenseFile)) {
  Write-Error "Missing $FontFile or $FontLicenseFile. Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF and upstream license at these paths."
}

.\scripts\build-ffmpeg-windows.ps1

$FfmpegFiles = @(
  "vendor\ffmpeg\windows\ffmpeg.exe",
  "vendor\ffmpeg\windows\BUILD_INFO.txt"
)

foreach ($FfmpegFile in $FfmpegFiles) {
  if (!(Test-Path $FfmpegFile)) {
    Write-Error "Missing $FfmpegFile after building bundled FFmpeg."
  }
}

cargo build --release
New-Item -ItemType Directory -Force -Path dist\windows | Out-Null
New-Item -ItemType Directory -Force -Path dist\windows\fonts | Out-Null
New-Item -ItemType Directory -Force -Path dist\windows\legal | Out-Null
Copy-Item target\release\mtd-subtitle-app.exe dist\windows\MTDSubtitleApp.exe
Copy-Item $FontFile dist\windows\fonts\
Copy-Item $NoticeFile dist\windows\legal\
Copy-Item $ThirdPartyFile dist\windows\legal\
Copy-Item $FontLicenseFile dist\windows\legal\
New-Item -ItemType Directory -Force -Path dist\windows\legal\ffmpeg | Out-Null
Copy-Item vendor\ffmpeg-src\LICENSE.md dist\windows\legal\ffmpeg\
Copy-Item vendor\ffmpeg-src\COPYING.LGPLv2.1 dist\windows\legal\ffmpeg\
Copy-Item vendor\ffmpeg-src\COPYING.LGPLv3 dist\windows\legal\ffmpeg\
Copy-Item vendor\ffmpeg\windows\BUILD_INFO.txt dist\windows\legal\ffmpeg\
Copy-Item vendor\ffmpeg\windows\ffmpeg.exe dist\windows\ffmpeg.exe
Write-Host "Build output: dist\windows"
