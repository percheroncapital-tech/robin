use crate::envelope::Envelope;

#[derive(Debug)]
pub enum ValidationError {
    NotJson(String),
    MissingField(&'static str),
    WrongType(&'static str),
    BadLevel(String),
}
impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::NotJson(e) => write!(f, "not json: {e}"),
            ValidationError::MissingField(k) => write!(f, "missing required field: {k}"),
            ValidationError::WrongType(k) => write!(f, "wrong type for field: {k}"),
            ValidationError::BadLevel(l) => write!(f, "invalid level: {l}"),
        }
    }
}
impl std::error::Error for ValidationError {}

const LEVELS: [&str; 5] = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

pub fn validate(line: &str) -> Result<Envelope, ValidationError> {
    let val: serde_json::Value =
        serde_json::from_str(line).map_err(|e| ValidationError::NotJson(e.to_string()))?;
    for k in ["v", "ts", "level", "service", "tenant", "target", "msg"] {
        if val.get(k).is_none() {
            return Err(ValidationError::MissingField(match k {
                "v" => "v", "ts" => "ts", "level" => "level", "service" => "service",
                "tenant" => "tenant", "target" => "target", _ => "msg",
            }));
        }
    }
    let level = val["level"].as_str().ok_or(ValidationError::WrongType("level"))?;
    if !LEVELS.contains(&level) {
        return Err(ValidationError::BadLevel(level.to_string()));
    }
    // deserialize into the typed envelope; unknown extra keys are ignored (additive-only safe)
    serde_json::from_value(val).map_err(|e| ValidationError::NotJson(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    const OK: &str = r#"{"v":1,"ts":"2026-06-19T23:00:00Z","level":"INFO","service":"redline","tenant":"bbt","target":"x","msg":"hi"}"#;

    #[test] fn accepts_conforming() { assert!(validate(OK).is_ok()); }

    #[test] fn accepts_unknown_extra_keys() {
        // additive-only: a newer emitter sending extra fields must still validate
        let line = r#"{"v":1,"ts":"t","level":"INFO","service":"s","tenant":"t","target":"x","msg":"m","brand_new_field":42}"#;
        assert!(validate(line).is_ok());
    }

    #[test] fn rejects_non_json() { assert!(matches!(validate("not json"), Err(ValidationError::NotJson(_)))); }

    #[test] fn rejects_missing_required() {
        let line = r#"{"v":1,"ts":"t","level":"INFO","service":"s","tenant":"t","target":"x"}"#; // no msg
        assert!(matches!(validate(line), Err(ValidationError::MissingField("msg"))));
    }

    #[test] fn rejects_bad_level() {
        let line = r#"{"v":1,"ts":"t","level":"LOUD","service":"s","tenant":"t","target":"x","msg":"m"}"#;
        assert!(matches!(validate(line), Err(ValidationError::BadLevel(_))));
    }
}
