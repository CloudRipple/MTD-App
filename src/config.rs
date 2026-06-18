use std::time::Duration;

pub(crate) const BASE_URL: &str = "https://studio.mosi.cn";
pub(crate) const DEFAULT_MODEL: &str = "moss-transcribe-diarize";
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(3);
pub(crate) const POLL_TIMEOUT: Duration = Duration::from_secs(1300);
pub(crate) const HARMONYOS_FONT_REGULAR: &str = "HarmonyOS_Sans_SC_Regular.ttf";

pub(crate) const MODELS: [&str; 4] = [
    "moss-transcribe-diarize",
    "moss-transcribe-diarize-20260325",
    "moss-transcribe-diarize-20260203",
    "moss-transcribe-diarize-20260101",
];
