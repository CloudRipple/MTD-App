$ErrorActionPreference = "Stop"
$FontFile = "assets\fonts\HarmonyOS_Sans_SC_Regular.ttf"
$NoticeFile = "NOTICE.md"
$ThirdPartyFile = "THIRD_PARTY_NOTICES.md"

if (!(Test-Path $FontFile)) {
  Write-Error "Missing $FontFile. Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF at this path."
}

cargo build --release
New-Item -ItemType Directory -Force -Path dist\windows | Out-Null
New-Item -ItemType Directory -Force -Path dist\windows\fonts | Out-Null
New-Item -ItemType Directory -Force -Path dist\windows\legal | Out-Null
Copy-Item target\release\mtd-subtitle-app.exe dist\windows\MTDSubtitleApp.exe
Copy-Item $FontFile dist\windows\fonts\
Copy-Item $NoticeFile dist\windows\legal\
Copy-Item $ThirdPartyFile dist\windows\legal\
if (Test-Path vendor\ffmpeg\windows\ffmpeg.exe) {
  Copy-Item vendor\ffmpeg\windows\ffmpeg.exe dist\windows\ffmpeg.exe
} elseif (Test-Path vendor\ffmpeg\ffmpeg.exe) {
  Copy-Item vendor\ffmpeg\ffmpeg.exe dist\windows\ffmpeg.exe
}
Write-Host "Build output: dist\windows"
