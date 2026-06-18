use std::sync::{Arc, Mutex};

use crate::models::JobSnapshot;

pub(crate) fn update_job(
    job: &Arc<Mutex<JobSnapshot>>,
    status: &str,
    progress: f32,
    preview: Option<String>,
) {
    let mut state = job.lock().expect("job lock");
    state.status = status.to_owned();
    state.progress = progress;
    if let Some(preview) = preview {
        state.preview = preview;
    }
}
