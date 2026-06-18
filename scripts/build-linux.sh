#!/usr/bin/env bash
set -euo pipefail

FONT_FILE="assets/fonts/HarmonyOS_Sans_SC_Regular.ttf"
NOTICE_FILE="NOTICE.md"
THIRD_PARTY_FILE="THIRD_PARTY_NOTICES.md"

if [ ! -f "$FONT_FILE" ]; then
  echo "Missing $FONT_FILE"
  echo "Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF at this path."
  exit 1
fi

cargo build --release
mkdir -p dist/linux
mkdir -p dist/linux/fonts dist/linux/legal
cp target/release/mtd-subtitle-app dist/linux/MTDSubtitleApp
chmod +x dist/linux/MTDSubtitleApp
cp "$FONT_FILE" dist/linux/fonts/
cp "$NOTICE_FILE" "$THIRD_PARTY_FILE" dist/linux/legal/
if [ -f vendor/ffmpeg/linux/ffmpeg ]; then
  cp vendor/ffmpeg/linux/ffmpeg dist/linux/ffmpeg
  chmod +x dist/linux/ffmpeg
elif [ -f vendor/ffmpeg/ffmpeg ]; then
  cp vendor/ffmpeg/ffmpeg dist/linux/ffmpeg
  chmod +x dist/linux/ffmpeg
fi
echo "Build output: dist/linux"
