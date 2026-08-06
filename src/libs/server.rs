use std::time::Duration;

use crate::libs::responses::{
    SubSonicErrorResponse, SubSonicPlaylistResponse, SubSonicSong, SubSonicStarredResponse,
};
use anyhow::{anyhow, Result};
use reqwest::blocking::Response;

pub struct Server {
    client: reqwest::blocking::Client,
    host: String,
    username: String,
    password: String,
}

impl Server {
    fn get(&self, endpoint: &str, params: Option<&String>) -> Result<Response> {
        let host = self.host.clone();
        let username = self.username.clone();
        let password = self.password.clone();

        let base_params = format!("v=1.16.1&c=rocksonic-rs&u={username}&p={password}");
        let url = match params {
            Some(params) => format!("{host}/{endpoint}?{base_params}&{params}"),
            None => format!("{host}/{endpoint}?{base_params}"),
        };
        let res = self.client.get(url).send()?;
        Ok(res)
    }

    fn test_connection(&self) -> Result<()> {
        let response = self.get("ping", None)?;
        let status = response.status();
        let text = response.text()?;
        let xml = serde_xml_rs::from_str::<SubSonicErrorResponse>(&text)
            .map_err(|_e| anyhow!(format!("status {}\n{}", status.to_string(), text)))?;
        if xml.status != "ok" {
            return match xml.error {
                Some(error) => Result::Err(anyhow!(error.message)),
                None => Result::Err(anyhow!(status)),
            };
        }
        Ok(())
    }

    pub fn get_cover_art(&self, id: &str, size: u16) -> Result<Response> {
        let response = self.get("getCoverArt", Some(&format!("id={}&size={}", id, size)))?;

        if let Some(content_type) = response.headers().get("Content-Type") {
            if content_type == "text/xml" {
                let xml = serde_xml_rs::from_str::<SubSonicErrorResponse>(&response.text()?)?;
                let error_message = xml.error.ok_or(anyhow!("unknown error"))?.message;
                return Result::Err(anyhow!(error_message));
            }
        };
        Ok(response)
    }

    pub fn get_song(&self, id: &str, mp3: Option<u16>) -> Result<Response> {
        let (endpoint, params) = song_request(id, mp3);
        let response = self.get(endpoint, Some(&params))?;

        if let Some(content_type) = response.headers().get("Content-Type") {
            if content_type == "text/xml" {
                let xml = serde_xml_rs::from_str::<SubSonicErrorResponse>(&response.text()?)?;
                let error_message = xml.error.ok_or(anyhow!("unknown error"))?.message;
                return Result::Err(anyhow!(error_message));
            }
        };
        Ok(response)
    }

    pub fn get_playlist(&self, playlist_id: &str) -> Result<SubSonicPlaylistResponse> {
        let response = self.get("getPlaylist", Some(&format!("id={}", playlist_id)))?;
        let xml = serde_xml_rs::from_str::<SubSonicPlaylistResponse>(&response.text()?)?;
        Ok(xml)
    }
    pub fn get_favs(&self) -> Result<Vec<SubSonicSong>> {
        let response = self.get("getStarred", None)?;
        let xml = serde_xml_rs::from_str::<SubSonicStarredResponse>(&response.text()?)?;
        Ok(xml.starred.songs)
    }

    pub fn connect(host: String, username: String, password: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        let server = Server {
            client,
            host,
            username,
            password,
        };
        server.test_connection().map(|()| server)
    }
}

fn song_request(id: &str, mp3: Option<u16>) -> (&'static str, String) {
    match mp3 {
        Some(bitrate) => ("stream", format!("id={id}&maxBitRate={bitrate}&format=mp3")),
        None => ("download", format!("id={id}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp3_requests_use_the_transcoding_endpoint() {
        assert_eq!(
            song_request("song-id", Some(192)),
            ("stream", "id=song-id&maxBitRate=192&format=mp3".to_string())
        );
    }

    #[test]
    fn original_format_requests_use_the_download_endpoint() {
        assert_eq!(
            song_request("song-id", None),
            ("download", "id=song-id".to_string())
        );
    }
}
