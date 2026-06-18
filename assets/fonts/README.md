# HarmonyOS Sans Font Files

This directory contains the UI font bundled with distributable packages:

```text
assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
assets/fonts/HarmonyOS_Sans_SC_LICENSE.txt
```

The files come from the official Huawei HarmonyOS design resource font package. Keep the upstream license file, `NOTICE.md`, and `THIRD_PARTY_NOTICES.md` in every distributed package.

The build scripts intentionally fail when the required font or license file is missing, because the app is expected to ship with both.
