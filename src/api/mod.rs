//! REST API routes for Taimen.
//!
//! Endpoints for room CRUD, participant management, and recording control.

pub mod rooms;

use axum::Router;

use crate::app_state::AppState;

/// Build the complete API router.
pub fn router() -> Router<AppState> {
    Router::new().nest("/api/v1/rooms", rooms::router())
}
