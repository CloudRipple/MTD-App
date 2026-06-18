use std::{
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use anyhow::{Context, Result, anyhow};
use reqwest::blocking::{Client, multipart};
use serde_json::{Value, json};

use crate::{
    config::{BASE_URL, POLL_INTERVAL, POLL_TIMEOUT},
    job::update_job,
    models::JobSnapshot,
};

pub(crate) fn upload_audio(
    client: &Client,
    api_key: &str,
    audio_path: &std::path::Path,
) -> Result<Value> {
    let form = multipart::Form::new()
        .file("file", audio_path)
        .with_context(|| format!("无法读取音频文件：{}", audio_path.display()))?;
    let response = client
        .post(format!("{BASE_URL}/api/v1/files/upload"))
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .context("上传音频失败")?
        .error_for_status()
        .context("MOSS 音频上传返回错误")?;
    Ok(response.json()?)
}

pub(crate) fn create_asr_task(
    client: &Client,
    api_key: &str,
    file_id: &str,
    model: &str,
    max_tokens: u32,
) -> Result<Value> {
    let payload = json!({
        "file_id": file_id,
        "model": model,
        "sampling_params": { "max_new_tokens": max_tokens }
    });
    let response = client
        .post(format!("{BASE_URL}/api/v1/asr/tasks"))
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .context("创建转写任务失败")?
        .error_for_status()
        .context("MOSS 创建任务返回错误")?;
    Ok(response.json()?)
}

pub(crate) fn poll_task(
    client: &Client,
    api_key: &str,
    task_id: &str,
    job: &Arc<Mutex<JobSnapshot>>,
) -> Result<Value> {
    let started = Instant::now();
    while started.elapsed() < POLL_TIMEOUT {
        let response = client
            .get(format!("{BASE_URL}/api/v1/asr/tasks/{task_id}"))
            .bearer_auth(api_key)
            .send()
            .context("查询转写任务失败")?
            .error_for_status()
            .context("MOSS 查询任务返回错误")?;
        let result: Value = response.json()?;
        match result.get("status").and_then(Value::as_str) {
            Some("SUCCESS") | Some("FAILED") => return Ok(result),
            Some(status) => {
                let progress = 58.0
                    + (started.elapsed().as_secs_f32() / POLL_TIMEOUT.as_secs_f32() * 20.0)
                        .min(20.0);
                update_job(job, &format!("MOSS 任务状态：{status}"), progress, None);
            }
            None => update_job(job, "MOSS 正在处理", 62.0, None),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(anyhow!("等待转写结果超时，请稍后检查任务状态：{task_id}"))
}
