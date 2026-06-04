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

        let result = self
            .osm
            .reverse_geocode(location.latitude.into(), location.longitude.into())
            .await
            .map_err(|e| {
                warn!(error = %e, "Nominatim reverse geocode failed");
                Status::unavailable(format!("reverse geocode request failed: {e}"))
            })?;

        let reverse_response = ReverseGeocodeResponse {
            street_number: result.street_number.unwrap_or_default(),
            street_name: result.street_name.unwrap_or_default(),
            municipality: result.municipality.unwrap_or_default(),
            country_subdivision: result.country_subdivision.unwrap_or_default(),
            country: result.country.unwrap_or_default(),
            postal_code: result.postal_code.unwrap_or_default(),
        };

        Ok(Response::new(EncryptedReverseGeocodeResponse {
            response: Some(EncryptedData::new(
                "humane.aibus.ReverseGeocodeResponse",
                reverse_response.encode_to_vec(),
            )),
        }))
    }
}
