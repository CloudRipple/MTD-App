#!/usr/bin/env bash
set -euo pipefail

FONT_FILE="assets/fonts/HarmonyOS_Sans_SC_Regular.ttf"
NOTICE_FILE="NOTICE.md"
THIRD_PARTY_FILE="THIRD_PARTY_NOTICES.md"
APP_DIR="dist/macos/MTDSubtitleApp.app"

if [ ! -f "$FONT_FILE" ]; then
  rm -rf "$APP_DIR"
  echo "Missing $FONT_FILE"
  echo "Removed stale $APP_DIR so an old app bundle cannot be opened by mistake."
  echo "Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF at this path."
  exit 1
fi

cargo build --release
mkdir -p dist/macos

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
mkdir -p "$APP_DIR/Contents/Resources/fonts" "$APP_DIR/Contents/Resources/legal"
cp target/release/mtd-subtitle-app "$APP_DIR/Contents/MacOS/MTDSubtitleApp"
chmod +x "$APP_DIR/Contents/MacOS/MTDSubtitleApp"
cp "$FONT_FILE" "$APP_DIR/Contents/Resources/fonts/"
cp "$NOTICE_FILE" "$THIRD_PARTY_FILE" "$APP_DIR/Contents/Resources/legal/"

cat > "$APP_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>MTDSubtitleApp</string>
  <key>CFBundleIdentifier</key>
  <string>cn.mtd.subtitle-app</string>
  <key>CFBundleName</key>
  <string>MTD Subtitle App</string>
  <key>CFBundleDisplayName</key>
  <string>MTD 字幕工作台</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

if [ -f vendor/ffmpeg/macos/ffmpeg ]; then
  cp vendor/ffmpeg/macos/ffmpeg "$APP_DIR/Contents/Resources/ffmpeg"
  chmod +x "$APP_DIR/Contents/Resources/ffmpeg"
elif [ -f vendor/ffmpeg/ffmpeg ]; then
  cp vendor/ffmpeg/ffmpeg "$APP_DIR/Contents/Resources/ffmpeg"
  chmod +x "$APP_DIR/Contents/Resources/ffmpeg"
fi

echo "Build output: $APP_DIR"
