use std::sync::atomic::Ordering;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::ApiState;
use crate::db::{ContactImportError, ContactRecord};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/contacts", get(list_contacts))
        .route("/contacts", post(create_contact))
        .route("/contacts/import", post(import_contacts))
        .route("/contacts/client-reset", post(client_reset))
        .route("/contacts/client-reset/claim", post(claim_client_reset))
        .route("/contacts/{id}", get(get_contact))
        .route("/contacts/{id}", put(update_contact))
        .route("/contacts/{id}", delete(delete_contact))
}

#[derive(Deserialize)]
struct ContactImportRequest {
    contacts: Vec<ContactRecord>,
}

async fn list_contacts(State(state): State<ApiState>) -> Response {
    match state.db.list_contacts().await {
        Ok(contacts) => Json(contacts).into_response(),
        Err(error) => {
            tracing::error!(%error, "failed to list contacts");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_contact(Path(id): Path<String>, State(state): State<ApiState>) -> Response {
    match state.db.get_contact(&id).await {
        Ok(Some(contact)) => Json(contact).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, id, "failed to get contact");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_contact(
    State(state): State<ApiState>,
    Json(contact): Json<ContactRecord>,
) -> Response {
    match state.db.upsert_contact(contact).await {
        Ok(contact) => {
            trigger_device_contact_sync().await;
            (StatusCode::CREATED, Json(contact)).into_response()
        }
        Err(error) => contact_write_error_response(error),
    }
}

async fn update_contact(
    Path(id): Path<String>,
    State(state): State<ApiState>,
    Json(mut contact): Json<ContactRecord>,
) -> Response {
    contact.id = id;
    match state.db.upsert_contact(contact).await {
        Ok(contact) => {
            trigger_device_contact_sync().await;
            Json(contact).into_response()
        }
        Err(error) => contact_write_error_response(error),
    }
}

async fn delete_contact(Path(id): Path<String>, State(state): State<ApiState>) -> Response {
    match state.db.delete_contact(&id).await {
        Ok(true) => {
            trigger_device_contact_sync().await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::error!(%error, id, "failed to delete contact");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn import_contacts(
    State(state): State<ApiState>,
    Json(body): Json<ContactImportRequest>,
) -> Response {
    match state.db.import_contacts_merge(body.contacts).await {
        Ok(summary) => {
            trigger_device_contact_sync().await;
            Json(summary).into_response()
        }
        Err(error) => contact_write_error_response(error),
    }
}

async fn client_reset(State(state): State<ApiState>) -> Response {
    state
        .contact_client_reset_pending
        .store(true, Ordering::SeqCst);

    trigger_device_contact_sync().await;

    Json(serde_json::json!({
        "reset_pending": true,
    }))
    .into_response()
}

async fn claim_client_reset(State(state): State<ApiState>) -> Response {
    let reset = state
        .contact_client_reset_pending
        .swap(false, Ordering::SeqCst);

    Json(serde_json::json!({ "reset": reset })).into_response()
}

async fn trigger_device_contact_sync() {
    #[cfg(target_os = "android")]
    {
        match tokio::process::Command::new("/system/bin/am")
            .args(["broadcast", "-a", "humane.central.debug.FORCE_CONTACT_SYNC"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                tracing::info!("requested device contact sync via broadcast");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(status = ?output.status, %stderr, "contact sync broadcast failed");
            }
            Err(error) => {
                tracing::warn!(%error, "failed to spawn contact sync broadcast");
            }
        }
    }
}

fn contact_write_error_response(error: Box<dyn std::error::Error + Send + Sync>) -> Response {
    if let Some(import_error) = error.downcast_ref::<ContactImportError>() {
        return (StatusCode::BAD_REQUEST, Json(import_error)).into_response();
    }

    tracing::error!(%error, "failed to write contact");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}
