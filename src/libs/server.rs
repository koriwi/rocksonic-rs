use std::time::Duration;

use crate::libs::responses::{SubSonicErrorResponse, SubSonicSong, SubSonicStarredResponse};
use anyhow::{anyhow, Result};
use reqwest::blocking::Response;

pub struct Server {
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
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .retry(reqwest::retry::for_host(self.host.clone()).max_retries_per_request(1000))
            .build()?;
        let res = client.get(url).send()?;
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

    pub fn get_song(&self, id: &str, mp3: Option<u16>) -> Result<Response> {
        let params = if let Some(bitrate) = mp3 {
            Some(&format!("id={}&maxBitRate={}&format=mp3", id, bitrate))
        } else {
            Some(&format!("id={}", id))
        };
        let response = self.get("download", params)?;

        if let Some(content_type) = response.headers().get("Content-Type") {
            if content_type == "text/xml" {
                let xml = serde_xml_rs::from_str::<SubSonicErrorResponse>(&response.text()?)?;
                let error_message = xml.error.ok_or(anyhow!("unknown error"))?.message;
                return Result::Err(anyhow!(error_message));
            }
        };
        Ok(response)
    }

    pub fn get_favs(&self) -> Result<Vec<SubSonicSong>> {
        let response = self.get("getStarred", None)?;
        let xml = serde_xml_rs::from_str::<SubSonicStarredResponse>(&response.text()?)?;
        Ok(xml.starred.songs)
    }

    pub fn connect(host: String, username: String, password: String) -> Result<Self> {
        let server = Server {
            host,
            username,
            password,
        };
        server.test_connection().map(|()| server)
    }
}
