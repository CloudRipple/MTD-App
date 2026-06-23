use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::{
    models::{JobSnapshot, Segment},
    subtitles::render_srt,
};

const PROJECT_VERSION: u64 = 1;
pub(crate) const PROJECT_FILE_NAME: &str = "project.mtd.json";

pub(crate) fn save_project(path: &Path, snapshot: &JobSnapshot) -> Result<()> {
    let value = json!({
        "version": PROJECT_VERSION,
        "status": snapshot.status,
        "progress": snapshot.progress,
        "task_id": snapshot.task_id,
        "file_id": snapshot.file_id,
        "usage": snapshot.usage,
        "preview": snapshot.preview,
        "include_speaker": snapshot.include_speaker,
        "done": snapshot.done,
        "output_dir": path_string(snapshot.output_dir.as_ref()),
        "input_media_path": path_string(snapshot.input_media_path.as_ref()),
        "input_video_path": path_string(snapshot.input_video_path.as_ref()),
        "srt_path": path_string(snapshot.srt_path.as_ref()),
        "vtt_path": path_string(snapshot.vtt_path.as_ref()),
        "subtitled_path": path_string(snapshot.subtitled_path.as_ref()),
        "segments": snapshot.segments.iter().map(segment_value).collect::<Vec<_>>(),
    });
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(path, bytes).with_context(|| format!("无法保存项目文件：{}", path.display()))
}

pub(crate) fn load_project(path: &Path) -> Result<JobSnapshot> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("无法读取项目文件：{}", path.display()))?;
    let value: Value = serde_json::from_str(&text).context("项目文件不是有效 JSON")?;
    let version = value.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version != PROJECT_VERSION {
        return Err(anyhow!("不支持的项目文件版本：{version}"));
    }

    let include_speaker = value
        .get("include_speaker")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let segments = parse_segments(value.get("segments"))?;
    let preview = value
        .get("preview")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| render_srt(&segments, include_speaker).unwrap_or_default());

    Ok(JobSnapshot {
        status: value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("已载入项目")
            .to_owned(),
        progress: value
            .get("progress")
            .and_then(Value::as_f64)
            .unwrap_or(100.0) as f32,
        task_id: string_field(&value, "task_id"),
        file_id: string_field(&value, "file_id"),
        usage: string_field(&value, "usage"),
        preview,
        segments,
        include_speaker,
        output_dir: path_field(&value, "output_dir")
            .or_else(|| path.parent().map(Path::to_path_buf)),
        input_media_path: path_field(&value, "input_media_path"),
        input_video_path: path_field(&value, "input_video_path"),
        srt_path: path_field(&value, "srt_path"),
        vtt_path: path_field(&value, "vtt_path"),
        project_path: Some(path.to_path_buf()),
        subtitled_path: path_field(&value, "subtitled_path"),
        done: value.get("done").and_then(Value::as_bool).unwrap_or(true),
        error: None,
    })
}

fn segment_value(segment: &Segment) -> Value {
    json!({
        "start": segment.start,
        "end": segment.end,
        "speaker": segment.speaker,
        "text": segment.text,
    })
}

fn parse_segments(value: Option<&Value>) -> Result<Vec<Segment>> {
    let Some(segments) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    segments
        .iter()
        .map(|segment| {
            Ok(Segment {
                start: segment.get("start").and_then(Value::as_f64).unwrap_or(0.0),
                end: segment.get("end").and_then(Value::as_f64).unwrap_or(0.0),
                speaker: segment
                    .get("speaker")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                text: segment
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            })
        })
        .collect()
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or("-")
        .to_owned()
}

fn path_string(path: Option<&PathBuf>) -> Option<String> {
    path.map(|path| path.display().to_string())
}

fn path_field(value: &Value, key: &str) -> Option<PathBuf> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_file_roundtrips_segments_and_paths() {
        let dir = std::env::temp_dir().join(format!(
            "mtd-project-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let project_path = dir.join(PROJECT_FILE_NAME);
        let snapshot = JobSnapshot {
            status: "完成".to_owned(),
            progress: 100.0,
            preview: "1\n00:00:00,000 --> 00:00:01,000\nS01: Hello\n".to_owned(),
            output_dir: Some(dir.clone()),
            input_media_path: Some(dir.join("input.mp4")),
            input_video_path: Some(dir.join("input.mp4")),
            srt_path: Some(dir.join("subtitles.srt")),
            project_path: Some(project_path.clone()),
            done: true,
            segments: vec![Segment {
                start: 0.0,
                end: 1.0,
                speaker: "S01".to_owned(),
                text: "Hello".to_owned(),
            }],
            ..JobSnapshot::default()
        };

        save_project(&project_path, &snapshot).unwrap();
        let loaded = load_project(&project_path).unwrap();

        assert_eq!(loaded.segments.len(), 1);
        assert_eq!(loaded.segments[0].speaker, "S01");
        assert_eq!(loaded.input_video_path, snapshot.input_video_path);
        assert_eq!(loaded.project_path, Some(project_path));
        let _ = fs::remove_dir_all(dir);
    }
}
