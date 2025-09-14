use crate::libs::responses::SubSonicErrorResponse;
use anyhow::{anyhow, Result};
use reqwest::blocking::Response;
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
        let res = reqwest::blocking::get(url)?;
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

    pub fn connect(host: String, username: String, password: String) -> Result<Self> {
        let server = Server {
            host,
            username,
            password,
        };
        server.test_connection().map(|()| server)
    }
}
