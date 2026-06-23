# Third-Party Notices

This file summarizes third-party assets and direct Rust dependencies used by MTD Subtitle App. It is not a substitute for reviewing the exact licenses of every transitive dependency in `Cargo.lock` before a public release.

## Bundled Assets

### HarmonyOS Sans SC Regular

- Type: Font
- Expected file: `assets/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- Upstream license file: `assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt`
- Distribution location:
  - macOS: `MTDSubtitleApp.app/Contents/Resources/fonts/HarmonyOS_Sans_SC_Regular.ttf`
  - Linux: `dist/linux/fonts/HarmonyOS_Sans_SC_Regular.ttf`
  - Windows: `dist/windows/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- License distribution location:
  - macOS: `MTDSubtitleApp.app/Contents/Resources/legal/HarmonyOS_Sans_SC_LICENSE.txt`
  - Linux: `dist/linux/legal/HarmonyOS_Sans_SC_LICENSE.txt`
  - Windows: `dist/windows/legal/HarmonyOS_Sans_SC_LICENSE.txt`
- Publisher: Huawei
- Notes: Public references describe HarmonyOS Sans as proprietary and free for commercial use. Keep the original upstream license from Huawei in every binary distribution.

### ffmpeg

- Type: External executable
- Source submodule: `vendor/ffmpeg-src`
- Expected files:
  - `vendor/ffmpeg/macos/ffmpeg`
  - `vendor/ffmpeg/linux/ffmpeg`
  - `vendor/ffmpeg/windows/ffmpeg.exe`
- Build scripts:
  - macOS/Linux: `scripts/build-ffmpeg.sh`
  - Windows: `scripts/build-ffmpeg-windows.ps1`
- Default configure flags: `--pkg-config-flags=--static --disable-gpl --disable-nonfree --enable-libass`
- Intended license profile: LGPL-compatible FFmpeg build. Do not enable GPL or nonfree FFmpeg components without updating the product license review and release notices.
- Distribution location:
  - macOS: `MTDSubtitleApp.app/Contents/Resources/ffmpeg`
  - Linux: `dist/linux/ffmpeg`
  - Windows: `dist/windows/ffmpeg.exe`
- License distribution location:
  - macOS: `MTDSubtitleApp.app/Contents/Resources/legal/ffmpeg/`
  - Linux: `dist/linux/legal/ffmpeg/`
  - Windows: `dist/windows/legal/ffmpeg/`
- Required release record: source URL, exact submodule commit, configure flags, license text, and any required offer for source code.

## Direct Rust Dependencies

The following direct crates are used by this application:

| Crate | Purpose | Common license |
| --- | --- | --- |
| `anyhow` | Error handling | MIT OR Apache-2.0 |
| `eframe` | Native desktop app framework | MIT OR Apache-2.0 |
| `egui` | Immediate-mode GUI | MIT OR Apache-2.0 |
| `reqwest` | HTTPS and multipart API client | MIT OR Apache-2.0 |
| `rfd` | Native file dialogs | MIT OR Apache-2.0 |
| `serde_json` | JSON parsing and writing | MIT OR Apache-2.0 |
| `winit` | Native window/event-loop integration on macOS | MIT OR Apache-2.0 |
| `objc2` | macOS Objective-C bridge for native menu integration | MIT |
| `objc2-app-kit` | AppKit bindings for native macOS menu integration | Zlib OR Apache-2.0 OR MIT |
| `objc2-foundation` | Foundation bindings for native macOS menu integration | MIT |

Before public distribution, generate full notices for transitive crates from `Cargo.lock` with a license-notice tool such as `cargo-about` or an equivalent internal compliance workflow.

## Runtime Services

### MOSS-Transcribe-Diarize API

The app calls the MOSS API configured by the user through their own API key. API terms and data-processing obligations are governed by the service provider's terms and the user's account agreement.
