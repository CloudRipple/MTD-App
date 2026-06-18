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
        let start = seconds_from_value(
            segment
                .get("start_s")
                .or_else(|| segment.get("start"))
                .or_else(|| segment.get("start_ms")),
        )?;
        let mut end = seconds_from_value(
            segment
                .get("end_s")
                .or_else(|| segment.get("end"))
                .or_else(|| segment.get("end_ms")),
        )?;
        if end <= start {
            end = start + 1.0;
        }
        normalized.push(Segment {
            start,
            end,
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
    let mut file = fs::File::create(path)?;
    for (index, segment) in segments.iter().enumerate() {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("{}: {}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            file,
            "{}\n{} --> {}\n{}\n",
            index + 1,
            srt_time(segment.start),
            srt_time(segment.end),
            text
        )?;
    }
    Ok(())
}

pub(crate) fn write_vtt(path: &Path, segments: &[Segment], include_speaker: bool) -> Result<()> {
    let mut file = fs::File::create(path)?;
    writeln!(file, "WEBVTT\n")?;
    for segment in segments {
        let text = if include_speaker && !segment.speaker.is_empty() {
            format!("<v {}>{}", segment.speaker, segment.text)
        } else {
            segment.text.clone()
        };
        writeln!(
            file,
            "{} --> {}\n{}\n",
            vtt_time(segment.start),
            vtt_time(segment.end),
            text
        )?;
    }
    Ok(())
}

fn seconds_from_value(value: Option<&Value>) -> Result<f64> {
    let Some(value) = value else {
        return Ok(0.0);
    };
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    let text = value.as_str().unwrap_or("0");
    let number = text
        .parse::<f64>()
        .with_context(|| format!("时间戳格式无效：{text}"))?;
    if number > 1000.0 && !text.contains('.') {
        Ok(number / 1000.0)
    } else {
        Ok(number)
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
