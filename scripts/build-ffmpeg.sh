#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="$ROOT_DIR/vendor/ffmpeg-src"
PLATFORM="${1:-}"

if [ -z "$PLATFORM" ]; then
  case "$(uname -s)" in
    Darwin) PLATFORM="macos" ;;
    Linux) PLATFORM="linux" ;;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows" ;;
    *)
      echo "Unsupported platform: $(uname -s)"
      exit 1
      ;;
  esac
fi

case "$PLATFORM" in
  macos|linux) EXE_NAME="ffmpeg" ;;
  windows) EXE_NAME="ffmpeg.exe" ;;
  *)
    echo "Unsupported ffmpeg build platform: $PLATFORM"
    exit 1
    ;;
esac

PREFIX_DIR="$ROOT_DIR/vendor/ffmpeg/$PLATFORM"
BUILD_DIR="$ROOT_DIR/build/ffmpeg/$PLATFORM"
STAMP_FILE="$PREFIX_DIR/.build-stamp"
BUILD_INFO_FILE="$PREFIX_DIR/BUILD_INFO.txt"
OUTPUT_FILE="$PREFIX_DIR/$EXE_NAME"
shift || true
EXTRA_CONFIGURE_FLAGS=("$@")

if [ ! -x "$SRC_DIR/configure" ]; then
  git -C "$ROOT_DIR" submodule update --init --depth 1 vendor/ffmpeg-src
fi

if [ ! -x "$SRC_DIR/configure" ]; then
  echo "Missing FFmpeg source submodule at vendor/ffmpeg-src"
  exit 1
fi

if ! command -v pkg-config >/dev/null 2>&1; then
  echo "pkg-config is required to build bundled ffmpeg."
  exit 1
fi

if ! pkg-config --exists libass; then
  echo "libass development files are required so the bundled ffmpeg supports subtitle burn-in."
  echo "macOS: brew install libass pkg-config"
  echo "Linux: install pkg-config and libass-dev/libass-devel"
  echo "Windows: use MSYS2 MinGW and install mingw-w64-x86_64-libass pkgconf"
  exit 1
fi

if [ -z "${FFMPEG_BUILD_JOBS:-}" ]; then
  if command -v sysctl >/dev/null 2>&1; then
    FFMPEG_BUILD_JOBS="$(sysctl -n hw.ncpu 2>/dev/null || echo 2)"
  elif command -v nproc >/dev/null 2>&1; then
    FFMPEG_BUILD_JOBS="$(nproc)"
  else
    FFMPEG_BUILD_JOBS="2"
  fi
fi

CONFIGURE_FLAGS=(
  "--prefix=$BUILD_DIR/install"
  "--pkg-config-flags=--static"
  "--disable-debug"
  "--disable-doc"
  "--disable-ffplay"
  "--disable-ffprobe"
  "--disable-gpl"
  "--disable-nonfree"
  "--disable-autodetect"
  "--enable-ffmpeg"
  "--enable-libass"
  "--enable-pic"
)

if [ "$PLATFORM" = "macos" ]; then
  CONFIGURE_FLAGS+=("--enable-videotoolbox")
fi

FFMPEG_COMMIT="$(git -C "$SRC_DIR" rev-parse HEAD 2>/dev/null || cat "$SRC_DIR/RELEASE")"
EXTRA_FLAGS_TEXT="${EXTRA_CONFIGURE_FLAGS[*]-}"
STAMP_VALUE="commit=$FFMPEG_COMMIT
platform=$PLATFORM
flags=${CONFIGURE_FLAGS[*]} $EXTRA_FLAGS_TEXT"

if [ -x "$OUTPUT_FILE" ] && [ -f "$STAMP_FILE" ] && [ "$(cat "$STAMP_FILE")" = "$STAMP_VALUE" ]; then
  echo "Bundled ffmpeg is up to date: $OUTPUT_FILE"
  exit 0
fi

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR" "$PREFIX_DIR"

(
  cd "$BUILD_DIR"
  CONFIGURE_COMMAND=("$SRC_DIR/configure" "${CONFIGURE_FLAGS[@]}")
  if [ "${#EXTRA_CONFIGURE_FLAGS[@]}" -gt 0 ]; then
    CONFIGURE_COMMAND+=("${EXTRA_CONFIGURE_FLAGS[@]}")
  fi
  "${CONFIGURE_COMMAND[@]}"
  make -j "$FFMPEG_BUILD_JOBS"
  make install
)

cp "$BUILD_DIR/install/bin/$EXE_NAME" "$OUTPUT_FILE"
chmod +x "$OUTPUT_FILE"

cat > "$STAMP_FILE" <<EOF
$STAMP_VALUE
EOF

cat > "$BUILD_INFO_FILE" <<EOF
FFmpeg source: https://git.ffmpeg.org/ffmpeg.git
FFmpeg commit: $FFMPEG_COMMIT
Platform: $PLATFORM
Configure flags:
${CONFIGURE_FLAGS[*]} $EXTRA_FLAGS_TEXT

This project builds FFmpeg with LGPL-compatible defaults:
- --disable-gpl
- --disable-nonfree
- --enable-libass for the subtitles filter used by burn-in output
EOF

echo "Built bundled ffmpeg: $OUTPUT_FILE"
