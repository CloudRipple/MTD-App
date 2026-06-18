# HarmonyOS Sans Font Files

Place the bundled UI font here before creating distributable packages:

```text
assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
```

Use the official Huawei HarmonyOS design resource download page as the source for HarmonyOS Sans. Keep the downloaded license or terms text in your release records, and keep `NOTICE.md` plus `THIRD_PARTY_NOTICES.md` in the distributed package.

The build scripts intentionally fail when the required font file is missing, because the app is expected to ship with this font.
