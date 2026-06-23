use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::{
    api::{create_asr_task, poll_task, upload_audio},
    job::update_job,
    media::{extract_audio, has_video_stream},
    media_types::is_direct_audio_path,
    models::JobSnapshot,
    project::{PROJECT_FILE_NAME, save_project},
    subtitles::{normalize_segments, render_srt_preview, write_srt, write_vtt},
};

pub(crate) fn run_job(
    job: &Arc<Mutex<JobSnapshot>>,
    media_path: PathBuf,
    output_root: PathBuf,
    api_key: String,
    model: String,
    max_tokens: u32,
    include_speaker: bool,
) -> Result<()> {
    let job_dir = output_root.join(format!("MTD字幕-{}", unix_timestamp()));
    fs::create_dir_all(&job_dir)
        .with_context(|| format!("无法创建输出目录：{}", job_dir.display()))?;

    let input_copy = job_dir.join(safe_filename(
        media_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("media"),
    ));
    update_job(job, "正在读取媒体信息", 8.0, None);
    let input_has_video = if is_direct_audio_path(&media_path) {
        false
    } else {
        has_video_stream(&media_path)?
    };

    let audio_path = job_dir.join("audio.m4a");
    let srt_path = job_dir.join("subtitles.srt");
    let vtt_path = job_dir.join("subtitles.vtt");
    let json_path = job_dir.join("transcript.json");
    let text_path = job_dir.join("transcript.txt");
    let project_path = job_dir.join(PROJECT_FILE_NAME);
    let subtitled_path = input_has_video.then(|| job_dir.join("subtitled.mp4"));

    let upload_audio_path = if input_has_video {
        update_job(job, "正在分离音频", 12.0, None);
        extract_audio(&media_path, &audio_path)?;
        audio_path.clone()
    } else {
        update_job(job, "正在准备音频", 12.0, None);
        media_path.clone()
    };

    update_job(job, "正在上传音频到 MOSS", 28.0, None);
    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let upload = upload_audio(&client, &api_key, &upload_audio_path)?;
    let file_id = upload
        .get("file_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("上传响应缺少 file_id"))?
        .to_owned();
    {
        let mut state = job.lock().expect("job lock");
        state.file_id = file_id.clone();
    }

    update_job(job, "正在创建转写任务", 42.0, None);
    let task = create_asr_task(&client, &api_key, &file_id, &model, max_tokens)?;
    let task_id = task
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("创建任务响应缺少 task_id"))?
        .to_owned();
    {
        let mut state = job.lock().expect("job lock");
        state.task_id = task_id.clone();
    }

    update_job(job, "正在后台归档媒体文件", 50.0, None);
    let archive_handle = archive_media(media_path, input_copy.clone());

    update_job(job, "MOSS 正在转写和区分说话人", 58.0, None);
    let result = poll_task(&client, &api_key, &task_id, job)?;
    if result.get("status").and_then(Value::as_str) == Some("FAILED") {
        let message = result
            .get("error_message")
            .and_then(Value::as_str)
            .unwrap_or("转写任务失败");
        return Err(anyhow!(message.to_owned()));
    }

    update_job(job, "正在生成字幕文件", 82.0, None);
    let result_text = result
        .get("result_text")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("转写结果缺少 result_text"))?;
    let transcript: Value =
        serde_json::from_str(result_text).context("无法解析 result_text JSON")?;
    let segments = normalize_segments(&transcript)?;
    if segments.is_empty() {
        return Err(anyhow!("转写成功，但没有可用字幕片段"));
    }

    update_job(job, "正在归档媒体文件", 80.0, None);
    finish_media_archive(archive_handle)?;

    fs::write(&json_path, serde_json::to_vec_pretty(&transcript)?)?;
    let full_text = transcript
        .get("full_text")
        .and_then(Value::as_str)
        .unwrap_or("");
    fs::write(&text_path, format!("{full_text}\n"))?;
    let has_invalid_times = segments.iter().any(|segment| segment.has_invalid_time());
    if !has_invalid_times {
        write_srt(&srt_path, &segments, include_speaker)?;
        write_vtt(&vtt_path, &segments, include_speaker)?;
    }

    let usage = result
        .get("usage")
        .and_then(|usage| usage.get("total_tokens"))
        .map(|value| format!("{value} total"))
        .unwrap_or_else(|| "-".to_owned());
    let preview = render_srt_preview(&segments, include_speaker);
    let snapshot = {
        let mut state = job.lock().expect("job lock");
        state.status = if has_invalid_times {
            "需要修正时间戳".to_owned()
        } else {
            "完成".to_owned()
        };
        state.progress = 100.0;
        state.usage = usage;
        state.preview = preview;
        state.segments = segments;
        state.include_speaker = include_speaker;
        state.output_dir = Some(job_dir);
        state.input_media_path = Some(input_copy.clone());
        state.input_video_path = input_has_video.then_some(input_copy);
        state.srt_path = Some(srt_path);
        state.vtt_path = Some(vtt_path);
        state.project_path = Some(project_path.clone());
        state.subtitled_path = subtitled_path;
        state.done = true;
        state.clone()
    };
    save_project(&project_path, &snapshot)?;
    Ok(())
}

fn archive_media(media_path: PathBuf, input_copy: PathBuf) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        fs::copy(&media_path, &input_copy)
            .with_context(|| format!("无法复制媒体到输出目录：{}", input_copy.display()))?;
        Ok(())
    })
}

fn finish_media_archive(handle: thread::JoinHandle<Result<()>>) -> Result<()> {
    handle
        .join()
        .map_err(|_| anyhow!("媒体归档线程异常退出"))??;
    Ok(())
}

fn safe_filename(name: &str) -> String {
    let mut output = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    let trimmed = output.trim_matches(['.', '_']);
    if trimmed.is_empty() {
        "media".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
