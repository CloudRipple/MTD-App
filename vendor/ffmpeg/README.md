# Bundled ffmpeg

Put platform-specific ffmpeg binaries here before building distributable packages.

```text
vendor/ffmpeg/macos/ffmpeg
vendor/ffmpeg/linux/ffmpeg
vendor/ffmpeg/windows/ffmpeg.exe
```

Build scripts copy the matching binary into the distributable output:

- macOS: `MTDSubtitleApp.app/Contents/Resources/ffmpeg`
- Linux: `dist/linux/ffmpeg`
- Windows: `dist/windows/ffmpeg.exe`

The app also supports `FFMPEG_PATH` for development and troubleshooting.

Check the license of the ffmpeg build you redistribute. Many builds are LGPL, while builds with GPL components require GPL-compatible redistribution terms.
