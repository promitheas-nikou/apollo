use std::path::{Path};
use std::sync::LazyLock;
use id3::{Tag, Timestamp, TagLike};
use rand::RngExt;
use regex::Regex;

#[allow(dead_code)]
pub struct ApolloMetadata {
    pub track_name: Option<String>,
    pub album_name: Option<String>,
    pub artist_name: Option<String>,
    pub genre: Option<String>,
    pub date: Option<Timestamp>,
    pub album_artist: Option<String>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub extension: Option<String>,
}

impl ApolloMetadata {
    pub(crate) fn empty() -> ApolloMetadata {
        ApolloMetadata{
            track_name: None,
            album_name: None,
            artist_name: None,
            genre: None,
            date: None,
            album_artist: None,
            disc_number: None,
            track_number: None,
            extension: None,
        }
    }
    pub fn get_artist_name(self: &ApolloMetadata) -> String {
        let s = self.album_artist.as_deref().unwrap_or(
            self.artist_name.as_deref().unwrap_or("Unknown Artist")
        );
        let r = s.split_once(DELIMITERS).map_or(s, |(first,_)| {first});
        r.to_string()
    }
}
pub fn truncate_utf8(mut s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk back to the nearest char boundary (at most 3 steps, since UTF-8 chars are ≤ 4 bytes).
    let mut idx = max_bytes;
    while !s.is_char_boundary(idx) {
        idx -= 1;
    }
    s.truncate(idx);
    s
}
fn sanitize_str(path_str: &str) -> String {
    // Compile regex only once for performance
    static INVALID_CHARS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"[?<>:"/\\|*\x00-\x1F]"#).unwrap()
    });
    let sanitized = INVALID_CHARS.replace_all(path_str.as_ref(), "_");

    let mut r = sanitized.trim().to_string();
    if r.is_empty() {
        return "unknown".to_string();
    }
    if r.ends_with('.') {
        r.push('_');
    }
    if r.starts_with('.') {
        r = format!("_{}", r);
    }
    truncate_utf8(r, 128)
}

fn produce_trackname(filepath: &Path, provisional_trackname: &str) -> Option<String> {
    if provisional_trackname.is_empty() || provisional_trackname=="unknown"{
        let temp_name = sanitize_str(&filepath.file_stem().unwrap().to_string_lossy());
        if temp_name.is_empty() || temp_name=="unknown" {
            Some(generate_random_trackname())
        }
        else {
            Some(temp_name)
        }
    }
    else {
        Some(provisional_trackname.to_string())
    }
}
fn delim_artist_string(path_str: &str) -> String {
    static DELIM_CHARS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"[/\\|]"#).unwrap()
    });
    let delimmed = DELIM_CHARS.replace_all(path_str, ";");
    delimmed.to_string()
}
pub fn extract_metadata(filepath: &Path) -> Option<ApolloMetadata> {
    let res = Tag::read_from_path(filepath);
    match res {
        Err(_) => None,
        Ok(tag) => {
            Some(ApolloMetadata {
                track_name: produce_trackname(filepath, sanitize_str(tag.title().unwrap_or("")).as_str()),
                album_name: tag.album().map(|album| sanitize_str(album)),
                date: tag.date_released(),
                genre: tag.genre().map(|genre| sanitize_str(genre)),
                disc_number: tag.disc(),
                track_number: tag.track(),
                album_artist: tag.album_artist().map(|artist| sanitize_str(delim_artist_string(artist).as_str())),
                artist_name: tag.artist().map(|artist| sanitize_str(delim_artist_string(artist).as_str())),
                extension: filepath.extension().and_then(|x| x.to_str()).map(|x| String::from(x)),
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
fn generate_random_trackname() -> String {
    let hex = generate_random_hex(6);
    format!("Unknown Track {hex}")
}

const DELIMITERS: &[char] = & ['|', '/', '\\', ';', ','];