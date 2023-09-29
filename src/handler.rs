use crate::registrator::{TrErr, create_transaction, confirm_transaction, abort_transaction};
use axum::{http::StatusCode, extract::Path, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use axum_macros::debug_handler;
// use log::{debug, info};

#[derive(Deserialize, Clone)]
pub struct MessageConfig {
    pub name: String,
    #[serde(default = "default_message_url")]
    pub subpath: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct TransactionMessage {
    pub services: Vec<MessageConfig>,
    pub data: Value,
}

#[derive(Deserialize)]
pub struct TransactionInfo {
    pub messages: Vec<TransactionMessage>,
    #[serde(default = "default_external_uuid")]
    pub external_uuid: Option<String>
}

fn default_message_url()-> Option<String> { None }
fn default_external_uuid()-> Option<String> { None }

#[derive(Serialize)]
pub struct ShortRespData {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>
}

pub async fn create(Json(payload): Json<TransactionInfo>) -> (StatusCode, Json<ShortRespData>) {
    match create_transaction(&payload) {
        Ok(id) => {
            (
                StatusCode::CREATED,
                Json(ShortRespData { status: "created", id: Some(id), message: None })
            )
        },
        Err(msg) => {
            (
                StatusCode::BAD_REQUEST,
                Json(ShortRespData { status: "failure", id: None, message: Some(msg) })
            )
        }
    }
}

#[debug_handler]
pub async fn confirm(Path((uid, service)): Path<(String, String)>) -> (StatusCode, String) {
    match confirm_transaction(&uid, &service).await {
        Ok(()) => (StatusCode::OK, "confirmed".to_string()),
        Err(TrErr::Repeated(e)) => (StatusCode::OK, e.to_string()),
        Err(TrErr::BadParams(e)) => (StatusCode::BAD_REQUEST, e.to_string()),
        Err(TrErr::Unprocessable(e)) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        Err(TrErr::Timeout(e)) => (StatusCode::REQUEST_TIMEOUT, e.to_string()),
        Err(TrErr::NotFound(e)) => (StatusCode::NOT_FOUND, e.to_string()),
        Err(TrErr::Aborted(e)) => (StatusCode::FAILED_DEPENDENCY, e.to_string()),
        Err(TrErr::Expired(e)) => (StatusCode::FAILED_DEPENDENCY, e.to_string())
    }
}

pub async fn abort(Path((uid, service)): Path<(String, String)>) -> (StatusCode, String) {
    match abort_transaction(&uid, &service) {
        Ok(()) => (StatusCode::OK, "aborted".to_string()),
        Err(TrErr::Aborted(e)) => (StatusCode::OK, e.to_string()),
        Err(TrErr::Repeated(e)) => (StatusCode::OK, e.to_string()),
        Err(TrErr::Expired(e)) => (StatusCode::BAD_REQUEST, e.to_string()),
        Err(TrErr::Unprocessable(e)) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        Err(TrErr::Timeout(e)) => (StatusCode::REQUEST_TIMEOUT, e.to_string()),
        Err(TrErr::BadParams(e)) => (StatusCode::BAD_REQUEST, e.to_string()),
        Err(TrErr::NotFound(e)) => (StatusCode::NOT_FOUND, e.to_string())
    }
}
