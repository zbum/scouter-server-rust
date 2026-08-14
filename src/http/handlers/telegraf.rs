use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::http::AppState;
use crate::http::error::HttpError;
use crate::protocol::pack::{ObjectPack, PerfCounterPack, Pack};
use crate::protocol::value::{MapValue, Value};
use crate::util::hash;

/// POST /telegraf/{obj_name} - Receive InfluxDB line protocol and create counter packs.
pub async fn handle_telegraf(
    State(state): State<AppState>,
    Path(obj_name): Path<String>,
    body: String,
) -> Result<StatusCode, HttpError> {
    let now = current_millis();
    let dummy_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let obj_name_full = format!("/{}", obj_name);
    let obj_hash = hash::hash(&obj_name_full);

    let mut line_count = 0;
    for line_str in body.lines() {
        let line_str = line_str.trim();
        if line_str.is_empty() || line_str.starts_with('#') {
            continue;
        }
        line_count += 1;
        if line_count > 1000 {
            tracing::warn!("Telegraf payload exceeded 1000 lines, truncating");
            break;
        }

        if let Some((measurement, fields)) = parse_influx_line(line_str) {
            let mut data = MapValue::new();
            for (key, value_str) in &fields {
                if let Some(num) = parse_influx_number(value_str) {
                    let counter_name = format!("{}_{}", measurement, key);
                    data.put(counter_name, Value::Float(num as f32));
                }
            }

            if !data.table.is_empty() {
                // Dispatch ObjectPack (heartbeat)
                let obj_pack = ObjectPack {
                    obj_type: measurement.clone(),
                    obj_hash,
                    obj_name: obj_name_full.clone(),
                    alive: true,
                    ..Default::default()
                };
                state
                    .dispatcher
                    .dispatch(Pack::Object(obj_pack), &dummy_addr)
                    .await;

                // Dispatch PerfCounterPack
                let perf_pack = PerfCounterPack {
                    time: now,
                    obj_name: obj_name_full.clone(),
                    timetype: 1, // REALTIME
                    data,
                };
                state
                    .dispatcher
                    .dispatch(Pack::PerfCounter(perf_pack), &dummy_addr)
                    .await;
            }
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Parse a single InfluxDB line protocol line.
/// Returns (measurement, fields) where fields is Vec<(key, value_string)>.
/// Format: measurement,tag1=val1 field1=val1,field2=val2 timestamp
fn parse_influx_line(line: &str) -> Option<(String, Vec<(String, String)>)> {
    // Find the first unescaped space to split measurement+tags from fields
    let first_space = find_unescaped(line, ' ')?;
    let measurement_tags = &line[..first_space];
    let rest = &line[first_space + 1..];

    // Extract measurement name (before first comma)
    let measurement = match measurement_tags.find(',') {
        Some(pos) => measurement_tags[..pos].to_string(),
        None => measurement_tags.to_string(),
    };

    // Extract fields (before optional second space which is the timestamp)
    let field_str = match find_unescaped(rest, ' ') {
        Some(pos) => &rest[..pos],
        None => rest,
    };

    // Parse fields
    let mut fields = Vec::new();
    for part in field_str.split(',') {
        if let Some(eq_pos) = part.find('=') {
            let key = part[..eq_pos].to_string();
            let mut value = part[eq_pos + 1..].to_string();
            // Remove quotes from string values
            if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value = value[1..value.len() - 1].to_string();
            }
            fields.push((key, value));
        }
    }

    if measurement.is_empty() {
        return None;
    }
    Some((measurement, fields))
}

/// Find the first occurrence of `ch` that is not preceded by a backslash.
fn find_unescaped(s: &str, ch: char) -> Option<usize> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == ch as u8 {
            if i == 0 || bytes[i - 1] != b'\\' {
                return Some(i);
            }
        }
    }
    None
}

/// Parse an InfluxDB field value to a numeric f64.
/// Values ending with 'i' are integers, pure numbers are floats, others are skipped.
fn parse_influx_number(value: &str) -> Option<f64> {
    if value.ends_with('i') {
        value[..value.len() - 1].parse::<i64>().ok().map(|v| v as f64)
    } else if value.ends_with('u') {
        value[..value.len() - 1].parse::<u64>().ok().map(|v| v as f64)
    } else {
        value.parse::<f64>().ok()
    }
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
