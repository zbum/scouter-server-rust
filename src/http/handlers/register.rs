use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::http::AppState;
use crate::http::error::HttpError;
use crate::http::models::RegisterRequest;

/// POST /register - Register counter metadata for an object type.
/// MVP: Accept and acknowledge. Full CounterManager is Phase 6+.
pub async fn handle_register(
    State(_state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<StatusCode, HttpError> {
    tracing::info!(
        "Counter registration: type={} counters={}",
        req.object.obj_type,
        req.counters.len()
    );

    for counter in &req.counters {
        tracing::debug!(
            "  Registered counter: name={} unit={} display={:?} total={} all={}",
            counter.name,
            counter.unit,
            counter.display,
            counter.total,
            counter.all,
        );
    }

    Ok(StatusCode::CREATED)
}
