use prost::Message as _;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

use super::envelope::unwrap_plaintext_data;
use crate::external::osm::OsmClient;
use crate::proto::aibus::*;
use crate::proto::common::encryption::{self, EncryptedData};

pub struct ReverseGeocodeHandler {
    osm: OsmClient,
}

impl ReverseGeocodeHandler {
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            osm: OsmClient::new(http_client),
        }
    }

    pub async fn encrypted_reverse_geocode(
        &self,
        request: Request<EncryptedReverseGeocodeRequest>,
    ) -> Result<Response<EncryptedReverseGeocodeResponse>, Status> {
        let req = request.into_inner();
        let location_bytes = unwrap_plaintext_data(&req.location)?;
        let location = encryption::LocationEnvelope::decode(location_bytes)
            .map_err(|e| Status::invalid_argument(format!("bad LocationEnvelope: {e}")))?;

        info!(
            lat = location.latitude,
            lon = location.longitude,
            ">>> EncryptedReverseGeocode"
        );

        let json = self
            .osm
            .reverse_geocode(location.latitude.into(), location.longitude.into())
            .await
            .map_err(|e| {
                warn!(error = %e, "Nominatim reverse geocode failed");
                Status::unavailable(format!("reverse geocode request failed: {e}"))
            })?;

        let address = json.get("address").unwrap_or(&serde_json::Value::Null);

        let field = |keys: &[&str]| -> String {
            keys.iter()
                .find_map(|key| address.get(*key).and_then(|value| value.as_str()))
                .unwrap_or("")
                .to_string()
        };

        let reverse_response = ReverseGeocodeResponse {
            street_number: field(&["house_number"]),
            street_name: field(&["road", "pedestrian", "footway", "path"]),
            municipality: field(&["city", "town", "village", "hamlet", "municipality"]),
            country_subdivision: field(&["state", "region", "county"]),
            country: field(&["country"]),
            postal_code: field(&["postcode"]),
        };

        Ok(Response::new(EncryptedReverseGeocodeResponse {
            response: Some(EncryptedData::new(
                "humane.aibus.ReverseGeocodeResponse",
                reverse_response.encode_to_vec(),
            )),
        }))
    }
}
