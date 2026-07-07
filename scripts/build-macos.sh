#!/usr/bin/env bash
set -euo pipefail

FONT_FILE="assets/fonts/HarmonyOS_Sans_SC_Regular.ttf"
FONT_LICENSE_FILE="assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt"
NOTICE_FILE="NOTICE.md"
THIRD_PARTY_FILE="THIRD_PARTY_NOTICES.md"
APP_DIR="dist/macos/MTDSubtitleApp.app"
CODESIGN_IDENTITY="${MACOS_CODESIGN_IDENTITY:--}"

bundle_macos_dylibs() {
  local binary="$1"
  local lib_dir="$2"
  local changed="1"

  mkdir -p "$lib_dir"

  while [ "$changed" = "1" ]; do
    changed="0"
    while IFS= read -r dep; do
      local dep_name
      dep_name="$(basename "$dep")"
      if [ ! -f "$lib_dir/$dep_name" ]; then
        cp "$dep" "$lib_dir/$dep_name"
        chmod u+w "$lib_dir/$dep_name"
        changed="1"
      fi
    done < <(
      find "$binary" "$lib_dir" -type f \( -name '*.dylib' -o -perm -111 \) -print0 |
        xargs -0 otool -L |
        awk '/^[[:space:]]*\/(opt\/homebrew|usr\/local)\// { print $1 }' |
        sort -u
    )
  done

  if [ -n "$(find "$lib_dir" -type f -name '*.dylib' -print -quit)" ]; then
    install_name_tool -add_rpath "@loader_path/lib" "$binary" 2>/dev/null || true

    while IFS= read -r bundled_lib; do
      local lib_name
      lib_name="$(basename "$bundled_lib")"
      install_name_tool -id "@rpath/$lib_name" "$bundled_lib"
      install_name_tool -add_rpath "@loader_path" "$bundled_lib" 2>/dev/null || true
    done < <(find "$lib_dir" -type f -name '*.dylib' | sort)

    while IFS= read -r original_dep; do
      local dep_name
      dep_name="$(basename "$original_dep")"
      install_name_tool -change "$original_dep" "@rpath/$dep_name" "$binary" 2>/dev/null || true
      while IFS= read -r mach_o; do
        install_name_tool -change "$original_dep" "@rpath/$dep_name" "$mach_o" 2>/dev/null || true
      done < <(find "$lib_dir" -type f -name '*.dylib' | sort)
    done < <(
      find "$binary" "$lib_dir" -type f \( -name '*.dylib' -o -perm -111 \) -print0 |
        xargs -0 otool -L |
        awk '/^[[:space:]]*\/(opt\/homebrew|usr\/local)\// { print $1 }' |
        sort -u
    )
  fi
}

sign_macos_file() {
  local target="$1"

  if [ "$CODESIGN_IDENTITY" = "-" ]; then
    codesign --force --sign - "$target"
  else
    codesign --force --options runtime --timestamp --sign "$CODESIGN_IDENTITY" "$target"
  fi
}

if [ ! -f "$FONT_FILE" ] || [ ! -f "$FONT_LICENSE_FILE" ]; then
  rm -rf "$APP_DIR"
  echo "Missing $FONT_FILE or $FONT_LICENSE_FILE"
  echo "Removed stale $APP_DIR so an old app bundle cannot be opened by mistake."
  echo "Download HarmonyOS Sans from the official Huawei design resource page, then place the Simplified Chinese regular TTF and upstream license at these paths."
  exit 1
fi

scripts/build-ffmpeg.sh macos
cargo build --release
mkdir -p dist/macos

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
mkdir -p "$APP_DIR/Contents/Resources/fonts" "$APP_DIR/Contents/Resources/legal"
cp target/release/mtd-subtitle-app "$APP_DIR/Contents/MacOS/MTDSubtitleApp"
chmod +x "$APP_DIR/Contents/MacOS/MTDSubtitleApp"
cp "$FONT_FILE" "$APP_DIR/Contents/Resources/fonts/"
cp "$NOTICE_FILE" "$THIRD_PARTY_FILE" "$FONT_LICENSE_FILE" "$APP_DIR/Contents/Resources/legal/"
mkdir -p "$APP_DIR/Contents/Resources/legal/ffmpeg"
cp vendor/ffmpeg-src/LICENSE.md \
  vendor/ffmpeg-src/COPYING.LGPLv2.1 \
  vendor/ffmpeg-src/COPYING.LGPLv3 \
  vendor/ffmpeg/macos/BUILD_INFO.txt \
  "$APP_DIR/Contents/Resources/legal/ffmpeg/"

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
  <string>MOSS Subtitle App</string>
  <key>CFBundleDisplayName</key>
  <string>MOSS 字幕工作台</string>
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

cp vendor/ffmpeg/macos/ffmpeg "$APP_DIR/Contents/Resources/ffmpeg"
chmod +x "$APP_DIR/Contents/Resources/ffmpeg"
bundle_macos_dylibs "$APP_DIR/Contents/Resources/ffmpeg" "$APP_DIR/Contents/Resources/lib"

while IFS= read -r bundled_lib; do
  sign_macos_file "$bundled_lib"
done < <(find "$APP_DIR/Contents/Resources/lib" -type f -name '*.dylib' | sort)
sign_macos_file "$APP_DIR/Contents/Resources/ffmpeg"

if [ "$CODESIGN_IDENTITY" = "-" ]; then
  codesign --force --deep --sign - "$APP_DIR"
  echo "Signed with ad-hoc identity. For direct launch on locked-down macOS systems, set MACOS_CODESIGN_IDENTITY to a valid Apple code-signing identity."
else
  codesign --force --deep --options runtime --timestamp --sign "$CODESIGN_IDENTITY" "$APP_DIR"
fi

echo "Build output: $APP_DIR"
