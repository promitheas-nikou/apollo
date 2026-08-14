use std::path::PathBuf;
use log::{debug, error, warn};
use crate::config::{ApolloConfig};


#[derive(Clone)]
pub struct ApolloDirectoryTask {
    pub dir: PathBuf
}

#[derive(Clone)]
pub struct ApolloCopyTask {
    pub src: PathBuf,
    pub dst: PathBuf,
}
pub fn issue_copy_task(task: ApolloCopyTask, conf: &mut ApolloConfig) {
    if conf.targets.contains(&task.dst) {
        warn!("Target already queued: {}", task.dst.display());
        return;
    }
    conf.copy_tasks.push(task.clone());
    conf.targets.insert(task.dst);
}

pub fn issue_dir_task(task: ApolloDirectoryTask, conf: &mut ApolloConfig) {
    if conf.dirs_queued.contains(&task.dir) {
        // warn!("Target directory already queued: {}", task.dir.display());
        return;
    }
    conf.dir_tasks.push(task.clone());
    conf.dirs_queued.insert(task.dir);
}
pub fn process_copy_task(task: &ApolloCopyTask, conf: &ApolloConfig) -> bool {
    // if !conf.dry_run {
    if task.dst.exists() {
        log::log!(conf.level_duplicate_file_alert, "File already exists: {}", task.dst.display());
        return true
    }
    match std::fs::copy(&task.src, &task.dst) {
        Ok(_) => {
            log::log!(conf.level_new_file_alert, "Copying file: {} -> {}", task.src.display(), task.dst.display());
            true
        }
        Err(e) => {
            error!("Encountered IO error while copying '{}' -> '{}': {}", task.src.display(), task.dst.display(), e.to_string());
            false
        }
    }
    // }
    // else {
    //     true
    // }
}

pub fn process_dir_task(task: &ApolloDirectoryTask, _conf: &ApolloConfig) -> bool {
    match std::fs::create_dir_all(&task.dir) {
        Ok(_) => {
            debug!("Created directory: '{}'", task.dir.display());
            true
        }
        Err(e) => {
            error!("Encountered IO error while creating directory '{}': {}", task.dir.display(), e.to_string());
            false
        }
    }
}
