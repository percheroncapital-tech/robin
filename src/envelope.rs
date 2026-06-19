use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENVELOPE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub v: u32,
    pub ts: String,
    pub level: String,
    pub service: String,
    pub tenant: String,
    pub target: String,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub span: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub fields: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serializes_required_fields_and_omits_empty_optionals() {
        let e = Envelope {
            v: ENVELOPE_VERSION,
            ts: "2026-06-19T23:00:00Z".into(),
            level: "INFO".into(),
            service: "redline".into(),
            tenant: "bbt".into(),
            target: "redline::http".into(),
            msg: "request handled".into(),
            request_id: Some("req-1".into()),
            trace_id: None,
            span: None,
            fields: BTreeMap::new(),
        };
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["v"], 1);
        assert_eq!(json["service"], "redline");
        assert_eq!(json["request_id"], "req-1");
        assert!(json.get("trace_id").is_none(), "empty optionals must be omitted");
    }
}
