use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestParams {
    pub request_type: String,
    pub value: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestPayload {
    pub method: String,
    pub params: Option<RequestParams>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttributePayload {
    pub lamp_state: bool,
    pub sensor_state: bool,
}
