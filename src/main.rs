use std::io::Write;
use std::collections::{HashSet};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use clap::{ArgAction, Parser};
use log;
use log::{debug, error, info, warn, Level, LevelFilter};
use rayon;
use rayon::prelude::*;
use regex::Regex;
use id3;
use id3::{Tag, TagLike, Timestamp};
use rand::RngExt;

#[derive(Clone)]
struct ApolloDirectoryTask {
    dir: PathBuf
}

#[derive(Clone)]
struct ApolloCopyTask {
    src: PathBuf,
    dst: PathBuf,
}

#[allow(dead_code)]
#[derive(Clone)]
struct ApolloConfig {
    prepend_tracknum: bool,
    split_in_album_directories: bool,
    write_playlist: bool,
    write_playlist_archive: bool,
    dry_run: bool,
    do_blacklist: bool,
    valid_number_delimiters: String,
    level_new_file_alert: Level,
    level_duplicate_file_alert: Level,
    output_directory: PathBuf,
    input_directory: PathBuf,
    blacklist_patterns: String,
    input_file_count: u32,
    output_files_count: u32,
    dirs_processed_count: u32,
    dir_tasks: Vec<ApolloDirectoryTask>,
    copy_tasks: Vec<ApolloCopyTask>,
    targets: HashSet<PathBuf>,
    dirs_queued: HashSet<PathBuf>,
    playlist: Vec<PathBuf>
}

#[allow(dead_code)]
impl ApolloConfig {
    fn default() -> ApolloConfig {
        ApolloConfig {
            prepend_tracknum: false,
            split_in_album_directories: false,
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
        }
    }
}

#[allow(dead_code)]
struct ApolloMetadata {
    track_name: Option<String>,
    album_name: Option<String>,
    artist_name: Option<String>,
    genre: Option<String>,
    date: Option<Timestamp>,
    album_artist: Option<String>,
    disc_number: Option<u32>,
    track_number: Option<u32>
}

impl ApolloMetadata {
    fn empty() -> ApolloMetadata {
        ApolloMetadata{
            track_name: None,
            album_name: None,
            artist_name: None,
            genre: None,
            date: None,
            album_artist: None,
            disc_number: None,
            track_number: None,
        }
    }
}

fn sanitize_str(path_str: &str) -> String {
    // Compile regex only once for performance
    static INVALID_CHARS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"[?<>:"/\\|*\x00-\x1F]"#).unwrap()
    });

    let sanitized = INVALID_CHARS.replace_all(path_str, "_");

    let r = sanitized.trim().to_string();

    // if r.ends_with('.') {
    //     r.push('_');
    // }

    r
}

fn extract_metadata(filepath: &Path) -> Option<ApolloMetadata> {
    let res = Tag::read_from_path(filepath);
    match res {
        Err(_) => None,
        Ok(tag) => {
            Some(ApolloMetadata {
                track_name: tag.title().map(|title| sanitize_str(title)),
                album_name: tag.album().map(|album| sanitize_str(album)),
                date: tag.date_released(),
                genre: tag.genre().map(|genre| sanitize_str(genre)),
                disc_number: tag.disc(),
                track_number: tag.track(),
                album_artist: tag.album_artist().map(|artist| sanitize_str(artist)),
                artist_name: tag.artist().map(|artist| sanitize_str(artist)),
            })
        }
    }
}

fn generate_random_hex(len: usize) -> String {
    let mut rng = rand::rng();

    let mut bytes = vec![0u8; (len+1)>>1];
    rng.fill(&mut bytes[..]);
    let mut ret = hex::encode(bytes);
    if len&1!=0 {
        ret.pop();
    }
    ret
}

#[allow(dead_code)]
fn generate_random_trackname(_conf: &ApolloConfig) -> String {
    let hex = generate_random_hex(6);
    format!("Unknown Track {hex}")
}
fn generate_file_path(conf: &ApolloConfig, data: &ApolloMetadata, original_path: &Path) -> PathBuf {
    // let mut path = conf.output_directory.clone();
    //
    // path.push(data.artist_name.as_deref().unwrap_or("Unknown Artist"));

    let mut path = PathBuf::from(data.artist_name.as_deref().unwrap_or("Unknown Artist"));

    if conf.split_in_album_directories {
        path.push(data.album_name.as_deref().unwrap_or("Unknown Album"));
    }

    let track_name = if let Some(name) = data.track_name.as_deref() {
        name
    } else {
        original_path.file_stem().map(|a| a.to_str()).flatten().unwrap_or("Unknown Track")
    };
    let extension = original_path.extension().map(|e| e.to_str()).flatten();
    let track_filename = if conf.prepend_tracknum {
        if let Some(t) = data.track_number {
            let track_num = t.to_string();
            format!("{track_num} {track_name}")
        } else {
            format!("__ {track_name}")
        }
    } else {
        track_name.to_string()
    };
    let filename = if let Some(ext) = extension {
        format!("{track_filename}.{ext}")
    } else {
        track_filename
    };

    path.push(filename);
    path
}
const MUSIC_SUFFIXES: [&str; 3] = ["mp3", "ogg", "wav"];
fn is_music_file(filepath: &Path, _conf: &ApolloConfig) -> bool{
    MUSIC_SUFFIXES.contains(&filepath.extension().map(|t| {t.to_str()}).flatten().unwrap_or(""))
}

fn format_song_name(data: &ApolloMetadata) -> String {
    format!("'{}', '{}' by '{}'", data.album_name.as_deref().unwrap_or("Unknown Album"), data.track_name.as_deref().unwrap_or("Unknown Track"), data.artist_name.as_deref().unwrap_or("Unknown Artist"))
}

fn issue_copy_task(task: ApolloCopyTask, conf: &mut ApolloConfig) {
    if conf.targets.contains(&task.dst) {
        warn!("Target already queued: {}", task.dst.display());
        return;
    }
    conf.copy_tasks.push(task.clone());
    conf.targets.insert(task.dst);
}

fn issue_dir_task(task: ApolloDirectoryTask, conf: &mut ApolloConfig) {
    if conf.dirs_queued.contains(&task.dir) {
        // warn!("Target directory already queued: {}", task.dir.display());
        return;
    }
    conf.dir_tasks.push(task.clone());
    conf.dirs_queued.insert(task.dir);
}

fn process_copy_task(task: &ApolloCopyTask, conf: &ApolloConfig) -> bool {
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

fn process_dir_task(task: &ApolloDirectoryTask, _conf: &ApolloConfig) -> bool {
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

fn process_file(filepath: &Path, conf: &mut ApolloConfig) {
    info!("Processing entry '{}'", filepath.display());
    if !filepath.exists() {
        error!("File does not exist: '{}'", filepath.display());
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
    let destination_abs = conf.output_directory.join(&destination);
    if conf.write_playlist {
        conf.playlist.push(destination);
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
    #[arg(short, long)]
    input: String,
    #[arg(short, long)]
    output: String,
    #[arg(long="prepend-track-number", action=ArgAction::SetTrue)]
    prepend_tracknum: bool,
    #[arg(long="dont-split-albums", action=ArgAction::SetTrue)]
    dont_split_album: bool,
    #[arg(short='l',long="create-playlist", action=ArgAction::SetTrue)]
    create_playlist: bool,
    #[arg(long="warn-duplicate", action=ArgAction::SetTrue)]
    warn_duplicate: bool,
    #[arg(long="warn-new", action=ArgAction::SetTrue)]
    warn_new: bool,
    #[arg(long="dry-run", action=ArgAction::SetTrue)]
    dry_run: bool,
    #[arg(long="archive-playlists", short='L', action=ArgAction::SetTrue)]
    playlist_archive: bool
}

fn generate_config_from_args(a: &ApolloArgs) -> ApolloConfig  {
    ApolloConfig {
        prepend_tracknum: a.prepend_tracknum,
        split_in_album_directories: !a.dont_split_album,
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
    }
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
    let playlist_name = conf.input_directory.file_name().unwrap().to_str().unwrap_or("Unknown Playlist");
    info!("Writing playlist with name '{}'", playlist_name);
    let playlist_filename = conf.output_directory.join(format!("Playlists/{playlist_name}.m3u"));
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

fn apollo_run_dir(conf: &mut ApolloConfig) {
    process_dir(&conf.input_directory.clone(), conf);
    let mut errors = 0;
    if !conf.dry_run {
        conf.dir_tasks.iter().for_each(|task| {
            if !process_dir_task(&task, &conf) {
                errors += 1;
            }
        });
        if errors > 0 {
            error!("Aborting due to errors while creating directories!");
        }
        let pool = ThreadPoolBuilder::new()
            .num_threads(8)
            .build()
            .unwrap();
        pool.install(||
            conf.copy_tasks.par_iter().for_each(|task| {
                if !process_copy_task(&task, &conf) {
                    //idk
                }
            }));
        if conf.write_playlist && !conf.dry_run {
            if let Err(e) = write_playlist_to_file(&conf) {
                error!("Error while creating playlist file!!! '{}'", e);
            }
        }
    }

}

fn main() {
    let args = ApolloArgs::parse();
    let mut conf = generate_config_from_args(&args);
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
    // let mut conf: ApolloConfig = ApolloConfig::default();
    // conf.output_directory = PathBuf::from("/home/promitheas/Downloads/test123/");
    // conf.input_directory = PathBuf::from("/home/promitheas/Downloads/Music/nightcore classics 2000/");
    let start = Instant::now();

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
        for entry in x {
            let entry = entry.unwrap();
            warn!("Processing playlist in {}!", entry.path().display());
            let mut new_conf = generate_config_from_args(&args);
            new_conf.input_directory = entry.path();
            let start = Instant::now();
            apollo_run_dir(&mut new_conf);
            let duration = start.elapsed();
            println!("Playlist: Processed {} files in {} directories in {:.3}ms", new_conf.input_file_count, new_conf.dirs_processed_count, duration.as_secs_f64() * 1000.0);
            conf.input_file_count += new_conf.input_file_count;
            conf.output_files_count += new_conf.output_files_count;
            conf.dirs_processed_count += new_conf.dirs_processed_count+1;
        }
    }
    else {
        apollo_run_dir(&mut conf);
    }

    let duration = start.elapsed();
    println!("Finished processing in: {:.3}ms", duration.as_secs_f64() * 1000.0);
    println!("Processed {} files in {} directories in {:.3}ms", conf.input_file_count, conf.dirs_processed_count, duration.as_secs_f64() * 1000.0);
}
