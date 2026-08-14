use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::Json;

use crate::http::AppState;
use crate::http::error::HttpError;
use crate::http::models::{CounterRequest, SuccessResponse};
use crate::protocol::pack::{ObjectPack, PerfCounterPack, Pack};
use crate::protocol::value::{MapValue, Value};
use crate::util::hash;

/// POST /counter - Receive JSON counter data and dispatch through the normal pipeline.
pub async fn handle_counter(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<SuccessResponse>, HttpError> {
    let requests: Vec<CounterRequest> = if body.is_array() {
        serde_json::from_value(body).map_err(|e| HttpError::BadRequest(e.to_string()))?
    } else {
        let single: CounterRequest =
            serde_json::from_value(body).map_err(|e| HttpError::BadRequest(e.to_string()))?;
        vec![single]
    };

    let dummy_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    for req in requests {
        let obj_name = build_obj_name(&req.object.host, req.object.name.as_deref());
        let obj_hash = hash::hash(&obj_name);
        let address = req.object.address.clone().unwrap_or_default();

        // Create and dispatch ObjectPack (heartbeat)
        let obj_pack = ObjectPack {
            obj_type: req.object.obj_type.clone(),
            obj_hash,
            obj_name: obj_name.clone(),
            address,
            alive: true,
            ..Default::default()
        };
        state
            .dispatcher
            .dispatch(Pack::Object(obj_pack), &dummy_addr)
            .await;

        // Create and dispatch PerfCounterPack
        if !req.counters.is_empty() {
            let now = current_millis();
            let mut data = MapValue::new();
            for c in &req.counters {
                data.put(&c.name, Value::Float(c.value as f32));
            }
            let perf_pack = PerfCounterPack {
                time: now,
                obj_name,
                timetype: 1, // REALTIME
                data,
            };
            state
                .dispatcher
                .dispatch(Pack::PerfCounter(perf_pack), &dummy_addr)
                .await;
        }
    }

    Ok(Json(SuccessResponse {
        success: true,
        message: "counters received".to_string(),
    }))
}

fn build_obj_name(host: &str, name: Option<&str>) -> String {
    match name {
        Some(n) if !n.is_empty() => format!("/{}/{}", host, n),
        _ => format!("/{}", host),
    }
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
