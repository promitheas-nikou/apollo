use std::path::{Path};
use std::sync::LazyLock;
use id3::{Tag, Timestamp, TagLike};
use rand::RngExt;
use regex::Regex;
use crate::config::ApolloConfig;

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

pub fn extract_metadata(filepath: &Path) -> Option<ApolloMetadata> {
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
fn generate_random_trackname(_conf: &ApolloConfig) -> String {
    let hex = generate_random_hex(6);
    format!("Unknown Track {hex}")
}

const DELIMITERS: &[char] = & ['|', '/', '\\', ';', ','];