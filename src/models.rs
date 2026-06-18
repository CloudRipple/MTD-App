use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct Segment {
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) speaker: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct JobSnapshot {
    pub(crate) status: String,
    pub(crate) progress: f32,
    pub(crate) task_id: String,
    pub(crate) file_id: String,
    pub(crate) usage: String,
    pub(crate) preview: String,
    pub(crate) output_dir: Option<PathBuf>,
    pub(crate) done: bool,
    pub(crate) error: Option<String>,
}

impl Default for JobSnapshot {
    fn default() -> Self {
        Self {
            status: "等待选择视频".to_owned(),
            progress: 0.0,
            task_id: "-".to_owned(),
            file_id: "-".to_owned(),
            usage: "-".to_owned(),
            preview: "生成后在这里预览 SRT。".to_owned(),
            output_dir: None,
            done: false,
            error: None,
        }
    }
}
