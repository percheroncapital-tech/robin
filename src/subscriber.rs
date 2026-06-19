use std::collections::BTreeMap;
use std::io::Write;
use tracing::field::{Field, Visit};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::envelope::{Envelope, ENVELOPE_VERSION};

pub struct ServiceCtx {
    pub service: &'static str,
    pub tenant: String,
}

// ── visitor ──────────────────────────────────────────────────────────────────

struct EventVisitor {
    msg: String,
    fields: BTreeMap<String, serde_json::Value>,
}

impl EventVisitor {
    fn new() -> Self {
        Self { msg: String::new(), fields: BTreeMap::new() }
    }
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        if field.name() == "message" {
            self.msg = s;
        } else {
            self.fields.insert(field.name().to_string(), serde_json::Value::String(s));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.msg = value.to_string();
        } else {
            self.fields.insert(field.name().to_string(), serde_json::Value::String(value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Number(value.into()));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if let Some(n) = serde_json::Number::from_f64(value) {
            self.fields.insert(field.name().to_string(), serde_json::Value::Number(n));
        } else {
            self.fields.insert(field.name().to_string(), serde_json::Value::String(value.to_string()));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
}

// ── span field visitor (for request_id / trace_id / span) ────────────────────

struct SpanFieldVisitor {
    request_id: Option<String>,
    trace_id: Option<String>,
    span: Option<String>,
}

impl SpanFieldVisitor {
    fn new() -> Self {
        Self { request_id: None, trace_id: None, span: None }
    }
}

impl Visit for SpanFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        match field.name() {
            "request_id" => self.request_id = Some(s),
            "trace_id"   => self.trace_id   = Some(s),
            "span"       => self.span        = Some(s),
            _ => {}
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "request_id" => self.request_id = Some(value.to_string()),
            "trace_id"   => self.trace_id   = Some(value.to_string()),
            "span"       => self.span        = Some(value.to_string()),
            _ => {}
        }
    }
}

// ── layer ─────────────────────────────────────────────────────────────────────

pub(crate) struct FleetLogLayer<W> {
    service: &'static str,
    tenant: String,
    writer: W,
}

impl<S, W> Layer<S> for FleetLogLayer<W>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        // collect event fields
        let mut visitor = EventVisitor::new();
        event.record(&mut visitor);

        // collect span fields (request_id, trace_id, span name)
        let mut span_fields = SpanFieldVisitor::new();
        if let Some(scope) = ctx.event_scope(event) {
            for span in scope {
                let ext = span.extensions();
                if let Some(fields) = ext.get::<StoredFields>() {
                    if span_fields.request_id.is_none() {
                        span_fields.request_id = fields.request_id.clone();
                    }
                    if span_fields.trace_id.is_none() {
                        span_fields.trace_id = fields.trace_id.clone();
                    }
                    if span_fields.span.is_none() {
                        span_fields.span = Some(span.name().to_string());
                    }
                }
            }
        }

        let level = event.metadata().level().as_str().to_uppercase();
        let target = event.metadata().target().to_string();

        let ts = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());

        let envelope = Envelope {
            v: ENVELOPE_VERSION,
            ts,
            level,
            service: self.service.to_string(),
            tenant: self.tenant.clone(),
            target,
            msg: visitor.msg,
            request_id: span_fields.request_id,
            trace_id: span_fields.trace_id,
            span: span_fields.span,
            fields: visitor.fields,
        };

        if let Ok(mut line) = serde_json::to_string(&envelope) {
            line.push('\n');
            let mut w = self.writer.make_writer();
            let _ = w.write_all(line.as_bytes());
        }
    }

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("span not found");
        let mut ext = span.extensions_mut();
        let mut stored = StoredFields::default();
        attrs.record(&mut stored);
        ext.insert(stored);
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("span not found");
        let mut ext = span.extensions_mut();
        if let Some(stored) = ext.get_mut::<StoredFields>() {
            values.record(stored);
        }
    }
}

// ── stored span fields ────────────────────────────────────────────────────────

#[derive(Default)]
struct StoredFields {
    request_id: Option<String>,
    trace_id: Option<String>,
}

impl Visit for StoredFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        match field.name() {
            "request_id" => self.request_id = Some(s),
            "trace_id"   => self.trace_id   = Some(s),
            _ => {}
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "request_id" => self.request_id = Some(value.to_string()),
            "trace_id"   => self.trace_id   = Some(value.to_string()),
            _ => {}
        }
    }
}

// ── public API ────────────────────────────────────────────────────────────────

pub(crate) fn layer_with_writer<W>(ctx: ServiceCtx, writer: W) -> FleetLogLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    FleetLogLayer { service: ctx.service, tenant: ctx.tenant, writer }
}

pub fn init(ctx: ServiceCtx) {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("LOG_FILTER")
        .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(layer_with_writer(ctx, std::io::stdout))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::prelude::*;

    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(b); Ok(b.len()) }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    impl<'a> MakeWriter<'a> for Buf {
        type Writer = Buf;
        fn make_writer(&'a self) -> Buf { self.clone() }
    }

    #[test]
    fn every_event_is_a_valid_envelope() {
        let buf = Buf::default();
        let ctx = ServiceCtx { service: "testsvc", tenant: "bbt".into() };
        let subscriber = tracing_subscriber::registry()
            .with(layer_with_writer(ctx, buf.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(user = "alice", "did a thing");
        });
        let out = String::from_utf8(buf.0.lock().unwrap().clone()).unwrap();
        let line = out.lines().next().expect("one log line");
        let env = validate(line).expect("emitted line must validate");
        assert_eq!(env.service, "testsvc");
        assert_eq!(env.tenant, "bbt");
        assert_eq!(env.msg, "did a thing");
        assert_eq!(env.fields.get("user").unwrap(), "alice");
    }
}
