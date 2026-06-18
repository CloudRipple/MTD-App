# Third-Party Notices

This file summarizes third-party assets and direct Rust dependencies used by MTD Subtitle App. It is not a substitute for reviewing the exact licenses of every transitive dependency in `Cargo.lock` before a public release.

## Bundled Assets

### HarmonyOS Sans SC Regular

- Type: Font
- Expected file: `assets/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- Distribution location:
  - macOS: `MTDSubtitleApp.app/Contents/Resources/fonts/HarmonyOS_Sans_SC_Regular.ttf`
  - Linux: `dist/linux/fonts/HarmonyOS_Sans_SC_Regular.ttf`
  - Windows: `dist/windows/fonts/HarmonyOS_Sans_SC_Regular.ttf`
- Publisher: Huawei
- Notes: Public references describe HarmonyOS Sans as proprietary and free for commercial use. Keep the original upstream terms from Huawei with release compliance records.

### ffmpeg

- Type: External executable
- Expected files:
  - `vendor/ffmpeg/macos/ffmpeg`
  - `vendor/ffmpeg/linux/ffmpeg`
  - `vendor/ffmpeg/windows/ffmpeg.exe`
- License: Depends on the selected binary build and configure flags. Common possibilities include LGPL and GPL.
- Required release record: source URL, version, configure flags, license text, and any required offer for source code.

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

Before public distribution, generate full notices for transitive crates from `Cargo.lock` with a license-notice tool such as `cargo-about` or an equivalent internal compliance workflow.

## Runtime Services

### MOSS-Transcribe-Diarize API

The app calls the MOSS API configured by the user through their own API key. API terms and data-processing obligations are governed by the service provider's terms and the user's account agreement.
