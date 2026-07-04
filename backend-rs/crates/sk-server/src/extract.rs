//! `AuthUser` extractor — the Rust equivalent of `@jwt_required()`.

use crate::error::ApiError;
use crate::state::SharedState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sk_auth::jwt::{decode_token, TokenType};
use sk_models::user::User;

pub struct AuthUser(pub User);

/// Extract the raw bearer token from `Authorization: Bearer <token>`.
pub fn bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

impl FromRequestParts<SharedState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            bearer(parts).ok_or_else(|| ApiError::unauthorized("Missing Authorization Header"))?;
        let claims = decode_token(
            &token,
            &state.config.jwt_secret_key,
            TokenType::Access,
            false,
        )
        .map_err(|e| ApiError::unauthorized(e.to_string()))?;
        let user_id = claims
            .sub
            .as_i64()
            .ok_or_else(|| ApiError::unauthorized("Invalid token subject"))?;
        let user = sk_models::user::find_by_id(&state.db, user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("User not found"))?;
        Ok(AuthUser(user))
    }
}

/// Like `AuthUser` but for the refresh endpoint (`@jwt_required(refresh=True)`).
pub struct RefreshUser(pub User);

impl FromRequestParts<SharedState> for RefreshUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token =
            bearer(parts).ok_or_else(|| ApiError::unauthorized("Missing Authorization Header"))?;
        let claims = decode_token(
            &token,
            &state.config.jwt_secret_key,
            TokenType::Refresh,
            false,
        )
        .map_err(|e| ApiError::unauthorized(e.to_string()))?;
        let user_id = claims
            .sub
            .as_i64()
            .ok_or_else(|| ApiError::unauthorized("Invalid token subject"))?;
        let user = sk_models::user::find_by_id(&state.db, user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("Invalid user"))?;
        Ok(RefreshUser(user))
    }
}
