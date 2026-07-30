use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::libs::{responses::SubSonicSong, utils};

pub fn song_output_path(
    song: &SubSonicSong,
    output_dir: &str,
    library_dir: &str,
    mp3: Option<u16>,
    flat: bool,
) -> String {
    let suffix = if mp3.is_some() || song.suffix == "opus" {
        String::from("mp3")
    } else {
        song.suffix.clone()
    };
    let library_path = Path::new(output_dir).join(library_dir);
    let output_path = if flat {
        let sanitized_song = utils::sanitize_filename(&format!(
            "{} {} {:0>3} {}.{}",
            song.artist,
            song.album,
            song.track.unwrap_or(0),
            song.title,
            suffix
        ));
        library_path.join(sanitized_song)
    } else {
        let sanitized_artist = utils::sanitize_filename(&song.artist);
        let sanitized_album = utils::sanitize_filename(&song.album);
        let sanitized_song = utils::sanitize_filename(&format!(
            "{:0>3} {}.{}",
            song.track.unwrap_or(0),
            song.title,
            suffix
        ));
        library_path
            .join(sanitized_artist)
            .join(sanitized_album)
            .join(sanitized_song)
    };
    output_path.to_string_lossy().into_owned()
}

pub fn remove_unlisted_songs(
    output_dir: &str,
    library_dir: &str,
    expected: &HashSet<PathBuf>,
    flat: bool,
) -> Result<()> {
    let lib_path = Path::new(output_dir).join(library_dir);
    if !fs::exists(&lib_path)? {
        return Ok(());
    }
    if flat {
        remove_from_flat_dir(&lib_path, expected)
    } else {
        remove_from_nested_dirs(&lib_path, expected)
    }
}

fn remove_from_flat_dir(lib_path: &Path, expected: &HashSet<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(lib_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && !expected.contains(&path) {
            fs::remove_file(&path)?;
            println!("removed {}", path.display());
        }
    }
    Ok(())
}

fn remove_from_nested_dirs(lib_path: &Path, expected: &HashSet<PathBuf>) -> Result<()> {
    for artist_entry in fs::read_dir(lib_path)? {
        let artist_entry = artist_entry?;
        let artist_path = artist_entry.path();
        if !artist_path.is_dir() {
            continue;
        }
        for album_entry in fs::read_dir(&artist_path)? {
            let album_entry = album_entry?;
            let album_path = album_entry.path();
            if !album_path.is_dir() {
                continue;
            }
            remove_unlisted_from_album(&album_path, expected)?;
            if !album_has_songs(&album_path)? {
                fs::remove_dir_all(&album_path)?;
            }
        }
        if fs::read_dir(&artist_path)?.next().is_none() {
            fs::remove_dir(&artist_path)?;
        }
    }
    Ok(())
}

fn remove_unlisted_from_album(album_path: &Path, expected: &HashSet<PathBuf>) -> Result<()> {
    for file_entry in fs::read_dir(album_path)? {
        let file_entry = file_entry?;
        let file_path = file_entry.path();
        if !file_path.is_file() {
            continue;
        }
        if file_path.file_name().and_then(|n| n.to_str()) == Some("cover.jpeg") {
            continue;
        }
        if !expected.contains(&file_path) {
            fs::remove_file(&file_path)?;
            println!("removed {}", file_path.display());
        }
    }
    Ok(())
}

fn album_has_songs(album_path: &Path) -> Result<bool> {
    let has_songs = fs::read_dir(album_path)?
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_str() != Some("cover.jpeg"));
    Ok(has_songs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_ignores_trailing_directory_separator() {
        let song = SubSonicSong {
            id: "1".into(),
            title: "Song".into(),
            track: Some(1),
            album: "Album".into(),
            artist: "Artist".into(),
            suffix: "flac".into(),
            size: 1,
        };

        assert_eq!(
            song_output_path(&song, "/tmp/music", "favs", None, false),
            song_output_path(&song, "/tmp/music/", "favs", None, false)
        );
    }
}
