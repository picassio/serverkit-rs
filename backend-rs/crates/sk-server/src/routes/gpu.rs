use crate::extract::AuthUser;
use axum::Json;
use serde_json::Value;

pub async fn info(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_gpu::info())
}
