# Bundled ffmpeg

FFmpeg source is tracked as a git submodule at:

```text
vendor/ffmpeg-src
```

Distribution scripts build FFmpeg from that source submodule and place platform-specific binaries here:

```text
vendor/ffmpeg/macos/ffmpeg
vendor/ffmpeg/linux/ffmpeg
vendor/ffmpeg/windows/ffmpeg.exe
```

These binaries are generated build artifacts and are intentionally ignored by git. Build scripts copy the matching binary into the distributable output:

- macOS: `MTDSubtitleApp.app/Contents/Resources/ffmpeg`
- Linux: `dist/linux/ffmpeg`
- Windows: `dist/windows/ffmpeg.exe`

The app also supports `FFMPEG_PATH` for development and troubleshooting, but packaged releases are expected to use the bundled binary.

## Building

Initialize the source submodule first:

```sh
git submodule update --init --depth 1 vendor/ffmpeg-src
```

Then run the platform package script. It will call `scripts/build-ffmpeg.sh` or `scripts/build-ffmpeg-windows.ps1` before copying files into `dist/`.

The default FFmpeg configure flags keep the build LGPL-compatible:

```text
--pkg-config-flags=--static --disable-gpl --disable-nonfree --enable-libass
```

`libass` is required because this app uses FFmpeg's `subtitles` filter for burn-in subtitle output.

On macOS, the package script also copies Homebrew/local-prefix runtime dylib dependencies into
`MTDSubtitleApp.app/Contents/Resources/lib`, rewrites them to `@rpath`, and re-signs them.

## Licensing

The package scripts copy FFmpeg license files and `BUILD_INFO.txt` into the distributable `legal/ffmpeg` directory. Keep the exact source commit, configure flags, and LGPL license texts with every release.
