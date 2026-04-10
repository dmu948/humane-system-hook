use tonic::{Request, Response, Status};
use tracing::info;

use crate::proto::featureflags::*;
use crate::proto::featureflags::feature_flags_service_server::FeatureFlagsService;

pub struct FeatureFlagsServiceImpl;

#[tonic::async_trait]
impl FeatureFlagsService for FeatureFlagsServiceImpl {
    async fn get_flags(
        &self,
        _request: Request<DeviceFeatureFlagRequest>,
    ) -> Result<Response<DeviceFeatureFlagResponse>, Status> {
        info!(">>> FeatureFlags.GetFlags");
        Ok(Response::new(DeviceFeatureFlagResponse {
            assignment: vec![],
        }))
    }
}
