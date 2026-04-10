//! Standalone gRPC server for Humane AI Pin.

mod config;
mod llm;
mod services;

/// Generated protobuf/gRPC modules.
mod proto {
    pub mod aibus {
        tonic::include_proto!("humane.aibus");
    }
    pub mod pushrelay {
        tonic::include_proto!("humane.pushrelay");
    }
    pub mod featureflags {
        tonic::include_proto!("humane.featureflags");
    }
    pub mod account {
        tonic::include_proto!("humane.account");
    }
    pub mod contacts {
        tonic::include_proto!("humane.contacts");
    }
    pub mod events {
        tonic::include_proto!("humane.events");
    }
}

use std::path::PathBuf;
use std::sync::Arc;

use proto::account::user_information_service_server::UserInformationServiceServer;
use proto::account::wifi_config_service_server::WifiConfigServiceServer;
use proto::aibus::ai_bus_service_server::AiBusServiceServer;
use proto::contacts::contacts_rpc_service_server::ContactsRpcServiceServer;
use proto::events::events_ingest_service_server::EventsIngestServiceServer;
use proto::featureflags::feature_flags_service_server::FeatureFlagsServiceServer;
use proto::pushrelay::push_relay_service_server::PushRelayServiceServer;

use services::aibus::AiBusServiceImpl;
use services::contacts::ContactsRpcServiceImpl;
use services::events::EventsIngestServiceImpl;
use services::featureflags::FeatureFlagsServiceImpl;
use services::pushrelay::PushRelayServiceImpl;
use services::user_info::UserInformationServiceImpl;
use services::wifi_config::WifiConfigServiceImpl;

use config::Config;
use llm::LlmAgent;
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Locate config file: check --config <path>, then ./config.toml, then next to binary
    let config_path = std::env::args()
        .position(|a| a == "--config")
        .and_then(|i| std::env::args().nth(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let config = Config::load(&config_path)?;

    // CLI port arg overrides config (for backward compat: `humane-server 9090`)
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.server.port);

    // Build LLM agent
    let agent = Arc::new(LlmAgent::from_config(
        &config.llm,
        &config.server.system_prompt,
    )?);

    let addr = format!("0.0.0.0:{}", port).parse()?;

    let provider_label = config.llm.provider.to_uppercase();
    info!("============================================================");
    info!("Humane gRPC server listening on {} (plaintext)", addr);
    info!(
        "LLM provider: {} (model: {})",
        provider_label, config.llm.model
    );
    info!("Services:");
    info!(
        "  - humane.aibus.AIBusService/Understand       ({})",
        config.llm.provider
    );
    info!("  - humane.pushrelay.PushRelayService/Subscribe (no-op hold)");
    info!("  - humane.pushrelay.PushRelayService/GetPushTokens (empty)");
    info!("  - humane.featureflags.FeatureFlagsService/GetFlags (empty)");
    info!("  - humane.account.WifiConfigService/ListSecureWifiConfigs (empty)");
    info!("  - humane.account.UserInformationService/GetUserPersonalDetails (stub)");
    info!("  - humane.contacts.ContactsRPCService/GetContacts (empty)");
    info!("  - humane.events.EventsIngestService/Ingest (discard)");
    info!("  - humane.events.EventsIngestService/IngestBatch (discard)");
    info!("  - All other RPCs: UNIMPLEMENTED");
    info!("============================================================");

    Server::builder()
        .add_service(AiBusServiceServer::new(AiBusServiceImpl { agent }))
        .add_service(PushRelayServiceServer::new(PushRelayServiceImpl))
        .add_service(FeatureFlagsServiceServer::new(FeatureFlagsServiceImpl))
        .add_service(WifiConfigServiceServer::new(WifiConfigServiceImpl))
        .add_service(UserInformationServiceServer::new(
            UserInformationServiceImpl,
        ))
        .add_service(ContactsRpcServiceServer::new(ContactsRpcServiceImpl))
        .add_service(EventsIngestServiceServer::new(EventsIngestServiceImpl))
        .serve(addr)
        .await?;

    Ok(())
}
