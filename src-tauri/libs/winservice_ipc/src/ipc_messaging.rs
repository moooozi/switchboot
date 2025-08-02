use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ClientRequest {
    pub id: String,
    pub payload: Vec<u8>, // or serde_json::Value, or a String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerResponse {
    pub id: String,
    pub status: String,
    pub result: Option<Vec<u8>>, // or Option<serde_json::Value>
    pub error: Option<String>,
}
