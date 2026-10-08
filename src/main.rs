mod config;
mod tasks;
pub mod metadata;
pub mod sorting;

use std::io::Write;
use std::collections::{HashSet};
use std::{fs, io};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
// use std::sync::LazyLock;
use clap::{ArgAction, Parser};
use log;
use log::{debug, error, info, warn, Level, LevelFilter};
use rayon;
use rayon::prelude::*;
// use regex::Regex;
// use rand::RngExt;
use config::ApolloConfig;
use tasks::ApolloCopyTask;
use tasks::ApolloDirectoryTask;
use metadata::ApolloMetadata;

const MUSIC_SUFFIXES: [&str; 3] = ["mp3", "ogg", "wav"];
fn is_music_file(filepath: &Path, _conf: &ApolloConfig) -> bool{
    MUSIC_SUFFIXES.contains(&filepath.extension().map(|t| {t.to_str()}).flatten().unwrap_or("").to_lowercase().as_str())
}

fn format_song_name(data: &ApolloMetadata) -> String {
    format!("'{}', '{}' by '{}'", data.album_name.as_deref().unwrap_or("Unknown Album"), data.track_name.as_deref().unwrap_or("Unknown Track"), data.artist_name.as_deref().unwrap_or("Unknown Artist"))
}

fn process_file(filepath: &Path, conf: &mut ApolloConfig) {
    info!("Processing entry '{}'", filepath.display());
    if !filepath.exists() {
        error!("File does not exist: '{}'", filepath.display());
        return;
    }
    if !is_music_file(filepath, conf) {
        warn!("Skipping non-music file: '{}'", filepath.display());
        return;
    }
    let metadata = extract_metadata(filepath);
    let data = if let Some(data) = metadata {
        data
    } else {
        error!("File missing metadata: '{}'", filepath.display());
        ApolloMetadata::empty()
    };
    conf.input_file_count += 1;
    info!("Interpreting as: {}", format_song_name(&data));
    let destination = generate_file_path(conf, &data, filepath);
    if let Err(e) = destination {
        error!("Failed to generate output file: '{}'", e);
        return
    }
    let correct_destination = destination.unwrap();
    info!("Moving to: {}", correct_destination.display());
    let destination_abs = conf.output_directory.join(&correct_destination);
    if conf.write_playlist {
        conf.playlist.push(correct_destination);
    }

    // if destination_abs.exists() {
    //     log::log!(conf.level_duplicate_file_alert, "File '{}' already exists, skipping...", destination.display());
    //     return;
    // } else {
    //     log::log!(conf.level_new_file_alert, "File '{}' doesn't exist yet, copying!", destination.display());
    // }
    issue_dir_task(ApolloDirectoryTask{
        dir: destination_abs.parent().unwrap().to_owned(),
    }, conf);

    issue_copy_task(ApolloCopyTask{
        src: PathBuf::from(filepath),
        dst: destination_abs.clone(),
    }, conf);
    conf.output_files_count += 1;
}

#[derive(Parser)]
#[command(version, about = option_env!("CARGO_PKG_DESCRIPTION"), author)]
// #[clap(APOLLO_AUTHOR, APOLLO_VERSION, )]
struct ApolloArgs {
    ///Input directory (recursively parsed)
    #[arg(short, long)]
    input: String,
    ///Output directory (root for output filenames)
    #[arg(short, long)]
    output: String,
    ///[DEPRECATED] Legacy format option, use --format instead
    #[arg(long="prepend-track-number", action=ArgAction::SetTrue)]
    prepend_tracknum: bool,
    ///[DEPRECATED] Legacy format option, use --format instead
    #[arg(long="dont-split-albums", action=ArgAction::SetTrue)]
    dont_split_album: bool,
    ///Warn for every DUPLICATE song found (does not warn for songs found only once)
    #[arg(long="warn-duplicate", action=ArgAction::SetTrue)]
    warn_duplicate: bool,
    ///Warn for every NEW song found (does not warn when it is found again)
    #[arg(long="warn-new", action=ArgAction::SetTrue)]
    warn_new: bool,
    ///Execute the full program BUT simulate executing filesystem operations
    /// (still prints full logs as if they have been performed but all actual work is skipped)
    #[arg(long="dry-run", action=ArgAction::SetTrue)]
    dry_run: bool,
    ///Parse each direct subdirectory of the input directory as a playlist
    #[arg(long="archive-playlists", short='L', action=ArgAction::SetTrue)]
    playlist_archive: bool,
    ///Also generate a .m3u8 file that contains all songs found in subdirectory (in the new format)
    #[arg(short='l',long="create-playlist", action=ArgAction::SetTrue)]
    create_playlist: bool,
    #[arg(long="format", default_value = None, help = "
Format string to generate output paths for music files. Valid format tokens are:
%T -> Track name
%a -> Album name
%A -> Artist name
%d -> Disc number
%n -> Track number
%% -> Literal '%' (percent) symbol
Use slashes to specify subdirectories (like normal)

Default format is \"%A/%a/%T\" or \"%A/%T\"
")]
    format: Option<String>,
    ///Proceed even if target output directory is NOT empty (isn't necessarily an issue, just to prevent mistakes, if you are sure of what you are doing, it is safe)
    #[arg(short='m',long="merge", action=ArgAction::SetTrue)]
    merge: bool
}

fn generate_config_from_args(a: &ApolloArgs) -> Option<ApolloConfig>  {
    // if a.test_mode {
    //     let mut test = String::new();
    //     let test_metadata = ApolloMetadata{
    //         track_name: Some("<track_name>".to_string()),
    //         album_artist: Some("<album_artist>".to_string()),
    //         artist_name: Some("<artist_name>".to_string()),
    //         album_name: Some("<album_name>".to_string()),
    //         track_number: Some(42),
    //         disc_number: Some(67),
    //         genre: Some("<genre>".to_string()),
    //         date: Some(Timestamp::from_str("19-01-2005").unwrap())
    //     };
    //     let pft: ApolloStringFormat = ApolloStringFormat::try_from("Track %d-%n %T by %A, %a").unwrap();
    //     pft.write(&test_metadata, &mut test).unwrap();
    //     println!("{}", test);
    //     exit(0);
    // }
    let backup_format: &str = if a.dont_split_album {
        "%A/%T"
    } else {
        "%A/%a/%T"
    };
    let format: &str = if let Some(format) = &a.format {
        format.as_str()
    } else {
        backup_format
    };
    let format_option = process_format_input_string(format);
    let Some(final_format) = format_option else {
        error!("Failed to process format string! Aborting!");
        return None
    };
    Some(ApolloConfig {
        prepend_tracknum: a.prepend_tracknum,
        valid_number_delimiters: ",_/;".to_string(),
        write_playlist: a.create_playlist || a.playlist_archive,
        write_playlist_archive: a.playlist_archive,
        level_new_file_alert: if a.warn_new {Level::Warn} else {Level::Debug},
        level_duplicate_file_alert: if a.warn_duplicate {Level::Warn} else {Level::Debug},
        output_directory: PathBuf::from(a.output.as_str()),
        input_directory: PathBuf::from(a.input.as_str()),
        do_blacklist: false,
        blacklist_patterns: "".to_string(),
        dirs_processed_count: 0,
        input_file_count: 0,
        output_files_count: 0,
        dry_run: a.dry_run,
        dir_tasks: vec![],
        copy_tasks: vec![],
        targets: HashSet::new(),
        dirs_queued: HashSet::new(),
        playlist: vec![],
        format: final_format,
        merge: a.merge
    })
}
fn process_dir(dir: &Path, conf: &mut ApolloConfig) {
    if !dir.exists() {
        error!("Directory does not exist: '{}'", dir.display());
        return
    }
    if !dir.is_dir() {
        error!("Input is not a directory: '{}'", dir.display());
        return
    }
    let x = dir.read_dir().unwrap();
    for entry in x {
        match entry {
            Ok(entry) => {
                // println!("{:?}", entry.path());
                let path = entry.path();
                if path.is_dir() {
                    debug!("Recursively entering subdirectory: '{}'", path.display());
                    process_dir(&path, conf);
                }
                else if path.is_file() {
                    // debug!("Processing file: '{}'", path.display());
                    process_file(&path, conf);
                }
            }
            Err(err) => {
                error!("Encountered unexpected IO error while iterating directory '{}': {}", dir.to_str().unwrap_or("???"), err.to_string());
            }
        }
    }
    conf.dirs_processed_count += 1;
}
use simple_logger::SimpleLogger;
use std::time::Instant;
use rayon::ThreadPoolBuilder;
use crate::metadata::{extract_metadata};
use crate::sorting::{generate_file_path, process_format_input_string};
use crate::tasks::{issue_copy_task, issue_dir_task, process_copy_task, process_dir_task};

fn make_unique_filename(filename: &PathBuf) -> PathBuf {
    if !filename.exists() {
        return filename.clone();
    }
    let stem = filename.file_stem().unwrap().to_str().unwrap();
    let ext = filename.extension().unwrap().to_str().unwrap();
    for i in 1 .. 1000 {
        let try_filename = filename.parent().unwrap().join(format!("{stem} ({i}).{ext}"));
        if !try_filename.exists() {
            return try_filename;
        }
    }
    assert!(!filename.exists()); //throw error
    filename.clone()
}

fn write_playlist_to_file(conf: &ApolloConfig) -> Result<(), std::io::Error> {
    let playlist_name = conf.input_directory.file_name().unwrap().to_string_lossy();//.unwrap_or("Unknown Playlist");
    info!("Writing playlist with name '{}'", playlist_name);
    let playlist_filename = conf.output_directory.join(format!("Playlists/{playlist_name}.m3u8"));
    let playlist_dir = playlist_filename.parent().unwrap();
    if !playlist_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&playlist_dir) {
            error!("Error while creating playlist directory: '{}'", e);
            return Err(e);
        }
    }
    let unique_playlist_filename = make_unique_filename(&playlist_filename);
    info!("Writing playlist to file '{}'", unique_playlist_filename.display());
    match std::fs::File::create(&unique_playlist_filename) {
        Ok(file) => {
            let mut bufw = BufWriter::new(file);
            if let Err(e) = write!(bufw, "#EXTM3U\n") {
                error!("Error while writing to playlist file: '{}'", e);
                return Err(e);
            }
            for e in &conf.playlist {
                let entry_str = e.to_string_lossy();

                let normalized_path = entry_str.replace('\\', "/");

                if let Err(e) = writeln!(bufw, "../{}", normalized_path) {
                    error!("Error while writing to playlist file: '{}'", e);
                    return Err(e);
                }
            }
            if let Err(e) = bufw.flush() {
                error!("Error while writing to playlist file: '{}'", e);
                return Err(e);
            }
        }
        Err(e) => {
            error!("Error creating file: '{}': {}", unique_playlist_filename.display(), e.to_string());
            return Err(e);
        }
    }
    Ok(())
}


fn is_dir_empty(path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(fs::read_dir(path)?.next().is_none())
}
fn apollo_run_dir(conf: &mut ApolloConfig) -> bool {
    if conf.output_directory.exists() {
        if conf.output_directory.is_dir() {
            if is_dir_empty(&conf.output_directory).unwrap() {
                info!("Output directory exists but is empty; proceeding");
            }
            else {
                if conf.merge {
                    warn!("Output directory exists and is not empty, still proceeding because of merge mode!");
                } else {
                    error!("Output directory exists but is not empty! Aborting!");
                    return false
                }
            }
        }
        else {
            error!("Output path is not a directory! Aborting!");
            return false
        }
    }
    else {
        info!("Output directory does not exist. Creating it...");
        if conf.dry_run {
            warn!("Output directory does not exists, but not creating it because this is a dry run...");
        }
        else {
            fs::create_dir_all(&conf.output_directory).unwrap();
        }
        info!("Output directory created! Proceeding...");
    }
    process_dir(&conf.input_directory.clone(), conf);
    let mut errors = 0;

    let dir_pb = indicatif::ProgressBar::new(conf.dir_tasks.len() as u64);
    conf.dir_tasks.iter().for_each(|task| {
        if conf.dry_run {
        std::thread::sleep(std::time::Duration::from_millis(1));
        }

        if !process_dir_task(&task, &conf) {
            errors += 1;
        }
        dir_pb.inc(1);
    });
    dir_pb.finish_and_clear();
    if errors > 0 {
        error!("Aborting due to errors while creating directories!");
        return false
    }
    let pool = ThreadPoolBuilder::new()
        .num_threads(8)
        .build()
        .unwrap();
    let copy_pb = indicatif::ProgressBar::new(conf.copy_tasks.len() as u64);
    pool.install(||
        conf.copy_tasks.par_iter().for_each(|task| {
            if conf.dry_run {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if !process_copy_task(&task, &conf) {
                error!("Error while copying file: '{}' -> '{}", task.src.display(), task.dst.display());
                //idk
            }
            copy_pb.inc(1);
        }));
    copy_pb.finish_and_clear();
    if conf.write_playlist && !conf.dry_run {
        if let Err(e) = write_playlist_to_file(&conf) {
            error!("Error while creating playlist file!!! '{}'", e);
            return false
        }
    }
    return true
}

fn main() {
    SimpleLogger::new().with_level(LevelFilter::Warn).init().unwrap();
    // let mut conf: ApolloConfig = ApolloConfig::default();
    // conf.output_directory = PathBuf::from("/home/promitheas/Downloads/test123/");
    // conf.input_directory = PathBuf::from("/home/promitheas/Downloads/Music/nightcore classics 2000/");
    let start = Instant::now();

    let args = ApolloArgs::parse();
    let maybe_conf = generate_config_from_args(&args);

    let Some(mut conf) = maybe_conf else {
        error!("Failed to parse arguments! Aborting!");
        return
    };
    if conf.write_playlist_archive {
        warn!("Proceeding in playlist archive mode!");
        let dir = &conf.input_directory;
        if !dir.exists() {
            error!("Directory does not exist: '{}'", dir.display());
            return
        }
        if !dir.is_dir() {
            error!("Input is not a directory: '{}'", dir.display());
            return
        }
        let x = dir.read_dir().unwrap();
        let mut should_merge = conf.merge;
        for entry in x {
            let entry = entry.unwrap();
            if !entry.path().is_dir() {
                continue;
            }
            warn!("Processing playlist in {}!", entry.path().display());
            let mut new_conf = generate_config_from_args(&args).unwrap();
            new_conf.input_directory = entry.path();
            new_conf.merge = should_merge;
            let start = Instant::now();
            if !apollo_run_dir(&mut new_conf) {
                error!("Error in subtask!");
                error!("Error while processing playlist directory {}", entry.path().display());
                error!("Aborting!");
                return;
            }
            should_merge = true;
            let duration = start.elapsed();
            println!("Playlist: Processed {} files in {} directories in {:.3}ms", new_conf.input_file_count, new_conf.dirs_processed_count, duration.as_secs_f64() * 1000.0);
            conf.input_file_count += new_conf.input_file_count;
            conf.output_files_count += new_conf.output_files_count;
            conf.dirs_processed_count += new_conf.dirs_processed_count;
        }
    }
    else {
        apollo_run_dir(&mut conf);
    }

    let duration = start.elapsed();
    println!("Finished processing in: {:.3}ms", duration.as_secs_f64() * 1000.0);
    println!("Processed {} files in {} directories in {:.3}ms", conf.input_file_count, conf.dirs_processed_count, duration.as_secs_f64() * 1000.0);
}
