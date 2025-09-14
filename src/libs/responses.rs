use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
pub struct SubSonicError {
    #[serde(rename = "@code")]
    pub code: u16,
    #[serde(rename = "@message")]
    pub message: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "subsonic-response")]
pub struct SubSonicErrorResponse {
    #[serde(rename = "@status")]
    pub status: String,
    pub error: Option<SubSonicError>,
}
