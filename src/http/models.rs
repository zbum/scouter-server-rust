use serde::{Deserialize, Serialize};

/// A single entry in the counter request array (or standalone object).
#[derive(Debug, Deserialize)]
pub struct CounterRequest {
    pub object: CounterObjectInfo,
    pub counters: Vec<CounterEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CounterObjectInfo {
    pub host: String,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub obj_type: String,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CounterEntry {
    pub name: String,
    pub value: f64,
}

/// Register request for dynamic counter registration.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub object: RegisterObjectInfo,
    pub counters: Vec<RegisterCounterInfo>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterObjectInfo {
    #[serde(rename = "type")]
    pub obj_type: String,
    pub display: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterCounterInfo {
    pub name: String,
    pub unit: String,
    pub display: Option<String>,
    #[serde(default = "default_true")]
    pub total: bool,
    #[serde(default = "default_true")]
    pub all: bool,
}

fn default_true() -> bool {
    true
}

/// Common success response.
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}
