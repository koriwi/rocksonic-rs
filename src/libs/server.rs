use anyhow::{anyhow, Result};
use reqwest::blocking::Response;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SubSonicError {
    #[serde(rename = "@code")]
    code: u16,
    #[serde(rename = "@message")]
    message: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "subsonic-response")]
pub struct SubSonicResponse {
    #[serde(rename = "@status")]
    status: String,
    error: SubSonicError,
}

pub struct Server {
    host: String,
    username: String,
    password: String,
}

impl Server {
    fn get(&self, endpoint: &str, params: Option<&str>) -> Result<Response> {
        let host = self.host.clone();
        let username = self.username.clone();
        let password = self.password.clone();

        let base_params = format!("v=1.16.1&c=rocksonic-rs&u={username}&p={password}");
        let url = match params {
            Some(params) => format!("{host}/{endpoint}?{base_params}&{params}"),
            None => format!("{host}/{endpoint}?{base_params}"),
        };
        println!("{url}");
        let res = reqwest::blocking::get(url)?;
        Ok(res)
    }

    fn test_connection(&self) -> Result<()> {
        let response = self.get("ping", None)?;
        let xml = serde_xml_rs::from_str::<SubSonicResponse>(&response.text()?)?;
        if xml.status != "ok" {
            return Result::Err(anyhow!(xml.error.message));
        }
        Ok(())
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
