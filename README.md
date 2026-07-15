# MOSS-Subtitle-Workbench

Rust native desktop subtitle workstation. It accepts audio or video input, submits
audio to MOSS-Transcribe-Diarize, writes SRT/VTT subtitles, and can burn subtitles
back into video files.

## Run From Source

```bash
cargo run
```

## Development Dependencies

- Rust 1.93+
- MOSS API Key
- FFmpeg for development, either available on `PATH` or configured with `FFMPEG_PATH`
- HarmonyOS Sans SC Regular at `assets/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- FFmpeg source submodule at `vendor/ffmpeg-src`

FFmpeg lookup order at runtime:

1. `FFMPEG_PATH`
2. `ffmpeg` or `ffmpeg.exe` beside the app
3. macOS `.app/Contents/Resources/ffmpeg`
4. embedded Windows FFmpeg extracted to the user cache
5. system `PATH`

## Build Distribution

Use the platform scripts for release builds:

```bash
scripts/build-macos.sh
scripts/build-linux.sh
```

Windows PowerShell:

```powershell
scripts\build-windows.ps1
```

Build outputs:

- `dist/macos/MOSS-Subtitle-Workbench.app`
- `dist/linux/MOSS-Subtitle-Workbench`
- `dist/windows/MOSS-Subtitle-Workbench.exe`

Windows distribution is a single executable. The app embeds FFmpeg, the required
MinGW runtime DLLs, and the UI font, then extracts runtime files into the user's
cache directory when needed. Do not publish extra DLL, font, or legal folders next
to the Windows exe.

## Bundled FFmpeg

FFmpeg source is managed as a git submodule:

```bash
git submodule update --init --depth 1 vendor/ffmpeg-src
```

Distribution scripts build FFmpeg for the current platform before packaging:

```text
vendor/ffmpeg/macos/ffmpeg
vendor/ffmpeg/linux/ffmpeg
vendor/ffmpeg/windows/ffmpeg.exe
```

macOS and Linux keep FFmpeg beside the app bundle/executable. Windows embeds
`ffmpeg.exe` and its MinGW DLL dependencies into `MOSS-Subtitle-Workbench.exe`.

Default FFmpeg flags stay LGPL-compatible:

```text
--pkg-config-flags=--static --disable-gpl --disable-nonfree --enable-libass
```

`libass` is required for the FFmpeg `subtitles` filter used by burn-in output.

Build dependencies:

- macOS: `brew install libass pkg-config`
- Linux: `pkg-config` and `libass-dev` or `libass-devel`
- Windows: MSYS2 MinGW with `mingw-w64-x86_64-libass`, `pkgconf`, GCC, nasm, and yasm

## Bundled Font

The UI uses HarmonyOS Sans SC Regular. The Windows build embeds the font into the
single exe. macOS and Linux package scripts copy it into their app resources.

Source files:

```text
assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt
```

`HARMONYOS_FONT_PATH` can override the bundled font during development.

## Release Workflow

GitHub Actions release builds produce one uploaded asset per platform. The Windows
job verifies that `dist/windows` contains only `MOSS-Subtitle-Workbench.exe` before upload.

## Output Files

New projects are created in `~/MOSS-Subtitle-Workbench/` by default, inside a
`MOSS-Subtitle-Workbench-<timestamp>/` project directory. The project root can be
changed in the app and is not changed when an existing project is opened.

Generated files include:

- `audio.m4a`: extracted audio for video input
- `transcript.json`: raw model result
- `transcript.txt`: full transcript text
- `subtitles.srt`: SRT subtitles
- `subtitles.vtt`: WebVTT subtitles
- `project.json`: editable app project snapshot
- `subtitled.mp4`: video with burned-in subtitles, when requested

Supported video inputs include MP4, MOV, MKV, WebM, M4V, and AVI. Supported audio
inputs include WAV, MP3, AAC, FLAC, OGG, MPEG, M4A, MP4 audio streams, WebM audio
streams, and PCM.

## API Flow

1. Video input is preprocessed with FFmpeg to extract audio; direct audio input is uploaded as-is.
2. `POST https://studio.mosi.cn/api/v1/files/upload` uploads the audio.
3. `POST https://studio.mosi.cn/api/v1/asr/tasks` creates an async ASR task.
4. The app polls `GET https://studio.mosi.cn/api/v1/asr/tasks/{task_id}` until completion.
5. The returned JSON is normalized into editable subtitle segments and written to disk.

## Legal Notes

Keep the license files in `assets/fonts`, `vendor/ffmpeg-src`, `NOTICE.md`, and
`THIRD_PARTY_NOTICES.md` up to date. The Windows single exe embeds redistributable
runtime files, so public/commercial release should still review the actual FFmpeg,
font, and Rust dependency license obligations.
