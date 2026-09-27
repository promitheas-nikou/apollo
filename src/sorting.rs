use std::error::Error;
use std::path::{Path, PathBuf};
use std::fmt::Write;
use log::{error, warn};
use crate::config::ApolloConfig;
use crate::metadata::ApolloMetadata;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ApolloFormatToken {
    TrackName,
    ArtistName,
    AlbumName,
    TrackNumber,
    DiscNumber,
    Literal(String)
}

impl ApolloFormatToken {
    pub fn write(&self, data: &ApolloMetadata, target: &mut String) -> Result<(), Box<dyn Error>> {
        match self {
            ApolloFormatToken::TrackName => {
                let name = data.track_name.as_deref()
                    .ok_or("track_name is missing")?;
                write!(target, "{name}")?;
            }
            ApolloFormatToken::ArtistName => {
                let name = data.artist_name.as_deref()
                    .ok_or("artist_name is missing")?;
                write!(target, "{name}")?;
            }
            ApolloFormatToken::AlbumName => {
                let name = data.album_name.as_deref()
                    .ok_or("album_name is missing")?;
                write!(target, "{name}")?;
            }
            ApolloFormatToken::TrackNumber => {
                let number = data.track_number
                    .ok_or("track_number is missing")?;
                write!(target, "{number}")?;
            }
            ApolloFormatToken::DiscNumber => {
                let number = data.disc_number
                    .ok_or("disc_number is missing")?;
                write!(target, "{number}")?;
            }
            ApolloFormatToken::Literal(value) => {
                write!(target, "{value}")?;
            }
            _ => return Err(format!("unimplemented token: {self:?}").into()),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ApolloStringFormat {
    pub tokens: Vec<ApolloFormatToken>
}

impl TryFrom<&str> for ApolloStringFormat {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut tks = Vec::<ApolloFormatToken>::new();
        let mut buf = String::with_capacity(64);
        let mut special = false;
        for (i, c) in s.char_indices() {
            if special {
                match c {
                    'T' => tks.push(ApolloFormatToken::TrackName),
                    'd' => tks.push(ApolloFormatToken::DiscNumber),
                    'n' => tks.push(ApolloFormatToken::TrackNumber),
                    'a' => tks.push(ApolloFormatToken::AlbumName),
                    'A' => tks.push(ApolloFormatToken::ArtistName),
                    _ => return Err(format!("unimplemented token: {c}").into()),
                }
                special = false;
            } else {
                if c=='%' {
                    special = true;
                    tks.push(ApolloFormatToken::Literal(buf));
                    buf = String::with_capacity(64);
                } else {
                    buf.push(c);
                }
            }
        }
        if special {
            return Err(format!("incomplete token").into());
        }
        if !buf.is_empty() {
            tks.push(ApolloFormatToken::Literal(buf.clone()));
        }
        Ok(ApolloStringFormat { tokens: tks })
    }
}

impl From<Vec<ApolloFormatToken>> for ApolloStringFormat {
    fn from(tokens: Vec<ApolloFormatToken>) -> Self {
        ApolloStringFormat { tokens }
    }
}

impl ApolloStringFormat {
    pub(crate) fn write(&self, data: &ApolloMetadata, target: &mut String) -> Result<(), Box<dyn Error>> {
        for token in &self.tokens {
            token.write(data, target)?
        }
        if let Some(suffix) = data.extension.as_ref() {
            write!(target, ".{suffix}")?;
        }
        Ok(())
    }
}

pub fn generate_file_path(conf: &ApolloConfig, data: &ApolloMetadata, original_path: &Path) -> Result<PathBuf,Box<dyn Error>> {
    // let mut path = conf.output_directory.clone();
    //
    // path.push(data.artist_name.as_deref().unwrap_or("Unknown Artist"));
    let mut buf = String::new();
    conf.format.write(data, &mut buf)?;
    Ok(PathBuf::from(buf))
    //
    // let mut path = PathBuf::from(data.get_artist_name());
    //
    // if conf.split_in_album_directories {
    //     path.push(data.album_name.as_deref().unwrap_or("Unknown Album"));
    // }
    //
    // let track_name = if let Some(name) = data.track_name.as_deref() {
    //     name
    // } else {
    //     original_path.file_stem().map(|a| a.to_str()).flatten().unwrap_or("Unknown Track")
    // };
    // let extension = original_path.extension().map(|e| e.to_str()).flatten();
    // let track_filename = if conf.prepend_tracknum {
    //     if let Some(t) = data.track_number {
    //         let track_num = t.to_string();
    //         format!("{track_num} {track_name}")
    //     } else {
    //         format!("__ {track_name}")
    //     }
    // } else {
    //     track_name.to_string()
    // };
    // let filename = if let Some(ext) = extension {
    //     format!("{track_filename}.{ext}")
    // } else {
    //     track_filename
    // };
    //
    // path.push(filename);
    // path
}

pub fn process_format_input_string(s: &str) -> Option<ApolloStringFormat> {
    let r = ApolloStringFormat::try_from(s);
    match r {
        Err(err) => {
            error!("Encountered error while parsing format string: {}", err);
            None
        },
        Ok(k) => {
            warn!("Successfully parsed custom format string! Overriding default filepath formatter");
            Some(k)
        }
    }
}