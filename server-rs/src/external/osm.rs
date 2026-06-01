//! Shared OpenStreetMap service client helpers.

use reqwest::RequestBuilder;

const OSM_USER_AGENT: &str = "PenumbraOS/0.1";
const NOMINATIM_REVERSE_URL: &str = "https://nominatim.openstreetmap.org/reverse";
const OVERPASS_API_URL: &str = "https://overpass-api.de/api/interpreter";

/// OpenStreetMap API client.
#[derive(Clone)]
pub struct OsmClient {
    http: reqwest::Client,
}

#[derive(Debug)]
pub enum OsmError {
    HttpRequest(reqwest::Error),
    ParseResponse(reqwest::Error),
}

impl std::fmt::Display for OsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpRequest(e) => write!(f, "OSM HTTP request failed: {e}"),
            Self::ParseResponse(e) => write!(f, "OSM response parse failed: {e}"),
        }
    }
}

impl std::error::Error for OsmError {}

impl OsmClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub async fn reverse_geocode(&self, lat: f64, lon: f64) -> Result<serde_json::Value, OsmError> {
        let url = format!("{NOMINATIM_REVERSE_URL}?format=jsonv2&lat={lat}&lon={lon}");

        execute_overpass_request(self.http.get(&url)).await
    }

    pub async fn overpass(&self, query: &str) -> Result<serde_json::Value, OsmError> {
        execute_overpass_request(self.http.post(OVERPASS_API_URL).form(&[("data", query)])).await
    }
}

async fn execute_overpass_request(builder: RequestBuilder) -> Result<serde_json::Value, OsmError> {
    builder
        .header(reqwest::header::USER_AGENT, OSM_USER_AGENT)
        .send()
        .await
        .map_err(OsmError::HttpRequest)?
        .error_for_status()
        .map_err(OsmError::HttpRequest)?
        .json()
        .await
        .map_err(OsmError::ParseResponse)
}
