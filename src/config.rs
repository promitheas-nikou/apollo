use std::collections::HashSet;
use std::path::PathBuf;
use log::{Level};
use crate::sorting::ApolloStringFormat;
use crate::tasks::ApolloDirectoryTask;
use crate::tasks::ApolloCopyTask;
#[allow(dead_code)]
#[derive(Clone)]
pub struct ApolloConfig {
    pub prepend_tracknum: bool,
    pub write_playlist: bool,
    pub write_playlist_archive: bool,
    pub dry_run: bool,
    pub do_blacklist: bool,
    pub valid_number_delimiters: String,
    pub level_new_file_alert: Level,
    pub level_duplicate_file_alert: Level,
    pub output_directory: PathBuf,
    pub input_directory: PathBuf,
    pub blacklist_patterns: String,
    pub input_file_count: u32,
    pub output_files_count: u32,
    pub dirs_processed_count: u32,
    pub dir_tasks: Vec<ApolloDirectoryTask>,
    pub copy_tasks: Vec<ApolloCopyTask>,
    pub targets: HashSet<PathBuf>,
    pub dirs_queued: HashSet<PathBuf>,
    pub playlist: Vec<PathBuf>,
    pub format: ApolloStringFormat,
    pub merge: bool
}

#[allow(dead_code)]
impl ApolloConfig {
    pub fn default() -> ApolloConfig {
        ApolloConfig {
            prepend_tracknum: false,
            valid_number_delimiters: "".to_string(),
            write_playlist: false,
            level_new_file_alert: Level::Error,
            level_duplicate_file_alert: Level::Error,
            output_directory: Default::default(),
            input_directory: Default::default(),
            do_blacklist: false,
            blacklist_patterns: "".to_string(),
            input_file_count: 0,
            output_files_count: 0,
            dirs_processed_count: 0,
            dry_run: true,
            write_playlist_archive: false,
            dir_tasks: vec![],
            copy_tasks: vec![],
            targets: HashSet::new(),
            dirs_queued: HashSet::new(),
            playlist: vec![],
            format: ApolloStringFormat::try_from("%A/%T").unwrap(),
            merge: false
        }
    }
}