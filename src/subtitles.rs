use std::{fs, io::Write, path::Path};

use anyhow::{Context, Result, anyhow};
use serde_json::Value;

use crate::models::Segment;

pub(crate) fn normalize_segments(transcript: &Value) -> Result<Vec<Segment>> {
    let segments = transcript
        .get("segments")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("模型结果缺少 segments 字段"))?;

    let mut normalized = Vec::new();
    for segment in segments {
        let text = segment
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        let fallback_start = normalized
            .last()
            .map(|segment: &Segment| segment.end)
            .unwrap_or(0.0);
        let start = time_from_value(
            segment
                .get("start_s")
                .or_else(|| segment.get("start"))
                .or_else(|| segment.get("start_ms")),
            fallback_start,
        );
        let mut end = time_from_value(
            segment
                .get("end_s")
                .or_else(|| segment.get("end"))
                .or_else(|| segment.get("end_ms")),
            start.seconds + 1.0,
        );
        if end.seconds <= start.seconds {
            end.seconds = start.seconds + 1.0;
        }
        normalized.push(Segment {
            start: start.seconds,
            end: end.seconds,
            raw_start: start.raw,
            raw_end: end.raw,
            start_valid: start.valid,
            end_valid: end.valid,
            speaker: segment
                .get("speaker")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned(),
            text: text.to_owned(),
        });
    }
    Ok(normalized)
}

pub(crate) fn write_srt(path: &Path, segments: &[Segment], include_speaker: bool) -> Result<()> {
    fs::write(path, render_srt(segments, include_speaker)?)?;
    Ok(())
}

pub(crate) fn write_vtt(path: &Path, segments: &[Segment], include_speaker: bool) -> Result<()> {
    fs::write(path, render_vtt(segments, include_speaker)?)?;
    Ok(())
}

pub(crate) fn render_srt(segments: &[Segment], include_speaker: bool) -> Result<String> {
    ensure_valid_times(segments)?;
    let mut output = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("{}: {}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            output,
            "{}\n{} --> {}\n{}\n",
            index + 1,
            srt_time(segment.start),
            srt_time(segment.end),
            text
        )?;
    }
    String::from_utf8(output).context("无法生成 SRT 文本")
}

pub(crate) fn render_vtt(segments: &[Segment], include_speaker: bool) -> Result<String> {
    ensure_valid_times(segments)?;
    let mut output = Vec::new();
    writeln!(output, "WEBVTT\n")?;
    for segment in segments {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("<v {}>{}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            output,
            "{} --> {}\n{}\n",
            vtt_time(segment.start),
            vtt_time(segment.end),
            text
        )?;
    }
    String::from_utf8(output).context("无法生成 VTT 文本")
}

pub(crate) fn render_srt_preview(segments: &[Segment], include_speaker: bool) -> String {
    let mut output = String::new();
    for (index, segment) in segments.iter().enumerate() {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("{}: {}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            preview_time(
                segment.raw_start.as_deref(),
                segment.start_valid,
                segment.start
            ),
            preview_time(segment.raw_end.as_deref(), segment.end_valid, segment.end),
            text
        ));
    }
    output
}

#[derive(Debug)]
struct ParsedTime {
    seconds: f64,
    raw: Option<String>,
    valid: bool,
}

fn time_from_value(value: Option<&Value>, fallback: f64) -> ParsedTime {
    let Some(value) = value else {
        return ParsedTime {
            seconds: fallback.max(0.0),
            raw: None,
            valid: true,
        };
    };
    if let Some(number) = value.as_f64() {
        return ParsedTime {
            seconds: normalize_numeric_seconds(number, ""),
            raw: None,
            valid: true,
        };
    }
    let text = value.as_str().unwrap_or("0");
    match text.parse::<f64>() {
        Ok(number) => ParsedTime {
            seconds: normalize_numeric_seconds(number, text),
            raw: None,
            valid: true,
        },
        Err(_) => ParsedTime {
            seconds: fallback.max(0.0),
            raw: Some(text.to_owned()),
            valid: false,
        },
    }
}

fn normalize_numeric_seconds(number: f64, text: &str) -> f64 {
    if number > 1000.0 && !text.contains('.') {
        number / 1000.0
    } else {
        number
    }
}

fn ensure_valid_times(segments: &[Segment]) -> Result<()> {
    if let Some(segment) = segments.iter().find(|segment| segment.has_invalid_time()) {
        let value = if !segment.start_valid {
            segment.raw_start.as_deref().unwrap_or("")
        } else {
            segment.raw_end.as_deref().unwrap_or("")
        };
        return Err(anyhow!("时间戳需要修正：{value}"));
    }
    Ok(())
}

fn preview_time(raw: Option<&str>, valid: bool, seconds: f64) -> String {
    if valid {
        srt_time(seconds)
    } else {
        raw.unwrap_or("").to_owned()
    }
}

fn srt_time(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let hours = millis / 3_600_000;
    let minutes = (millis % 3_600_000) / 60_000;
    let secs = (millis % 60_000) / 1000;
    let ms = millis % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02},{ms:03}")
}

fn vtt_time(seconds: f64) -> String {
    srt_time(seconds).replace(',', ".")
}

#[cfg(test)]
mod tests {
    use super::{normalize_segments, render_srt, render_srt_preview};
    use crate::models::Segment;
    use serde_json::json;

    #[test]
    fn renders_speaker_name_into_srt_text() {
        let segments = vec![Segment {
            start: 2.54,
            end: 4.49,
            raw_start: None,
            raw_end: None,
            start_valid: true,
            end_valid: true,
            speaker: "张三".to_owned(),
            text: "你好".to_owned(),
        }];

        let srt = render_srt(&segments, true).expect("render srt");

        assert!(srt.contains("张三: 你好"));
    }

    #[test]
    fn preserves_invalid_timestamp_for_repairable_preview() {
        let transcript = json!({
            "segments": [
                { "start": "02:46.83", "end": 170.0, "text": "hello" }
            ]
        });

        let segments = normalize_segments(&transcript).expect("normalize");

        assert_eq!(segments.len(), 1);
        assert!(!segments[0].start_valid);
        assert_eq!(segments[0].raw_start.as_deref(), Some("02:46.83"));
        assert!(render_srt(&segments, false).is_err());
        assert!(render_srt_preview(&segments, false).contains("02:46.83 -->"));
    }
}
