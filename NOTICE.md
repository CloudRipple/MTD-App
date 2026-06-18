# Notices

MTD Subtitle App

This product includes third-party components and assets. Keep this notice and `THIRD_PARTY_NOTICES.md` with every binary distribution.

## HarmonyOS Sans

This product bundles HarmonyOS Sans SC Regular for application UI text rendering.

- Font family: HarmonyOS Sans / HarmonyOS Sans SC
- Copyright holder / publisher: Huawei, with typeface design attributed publicly to Hanyi Fonts
- Source: Huawei HarmonyOS Design Resources
- Publicly documented status: HarmonyOS Sans is described as a proprietary font that is available for free commercial use from Huawei's developer/design resources.

HarmonyOS Sans is not treated as an open-source font in this project. Do not redistribute modified font files unless the upstream license or written permission allows it. Keep the upstream font license or official terms obtained with the downloaded font package in release compliance records.

## ffmpeg

This product may bundle a platform-specific ffmpeg executable to extract audio and burn subtitles.

ffmpeg license obligations depend on the exact build that is bundled. Many ffmpeg builds are LGPL, while builds compiled with GPL components require GPL-compatible redistribution. Before shipping a release, record the ffmpeg source, version, configure flags, and license files for the bundled binary.

## Rust Dependencies

This product uses Rust crates listed in `Cargo.lock`. Direct dependencies are listed in `THIRD_PARTY_NOTICES.md`; transitive dependency notices should be generated and reviewed before public release.
