#!/usr/bin/env bash
set -euo pipefail

FONT_FILE="assets/fonts/HarmonyOS_Sans_SC_Regular.ttf"
FONT_LICENSE_FILE="assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt"
NOTICE_FILE="NOTICE.md"
THIRD_PARTY_FILE="THIRD_PARTY_NOTICES.md"

if [ ! -f "$FONT_FILE" ] || [ ! -f "$FONT_LICENSE_FILE" ]; then
  echo "Missing $FONT_FILE or $FONT_LICENSE_FILE"
  echo "Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF and upstream license at these paths."
  exit 1
fi

scripts/build-ffmpeg.sh linux
cargo build --release
mkdir -p dist/linux
mkdir -p dist/linux/fonts dist/linux/legal
cp target/release/mtd-subtitle-app dist/linux/MOSS-Subtitle-Workbench
chmod +x dist/linux/MOSS-Subtitle-Workbench
cp "$FONT_FILE" dist/linux/fonts/
cp "$NOTICE_FILE" "$THIRD_PARTY_FILE" "$FONT_LICENSE_FILE" dist/linux/legal/
mkdir -p dist/linux/legal/ffmpeg
cp vendor/ffmpeg-src/LICENSE.md \
  vendor/ffmpeg-src/COPYING.LGPLv2.1 \
  vendor/ffmpeg-src/COPYING.LGPLv3 \
  vendor/ffmpeg/linux/BUILD_INFO.txt \
  dist/linux/legal/ffmpeg/
cat > dist/linux/MOSS-Subtitle-Workbench.desktop <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=MOSS-Subtitle-Workbench
Name[zh_CN]=MOSS-Subtitle-Workbench
Exec=MOSS-Subtitle-Workbench
Terminal=false
Categories=AudioVideo;Utility;
DESKTOP
cp vendor/ffmpeg/linux/ffmpeg dist/linux/ffmpeg
chmod +x dist/linux/ffmpeg
echo "Build output: dist/linux"
