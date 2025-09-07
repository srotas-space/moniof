#![cfg(feature = "sqlx")]

use crate::stats::{normalize_sql, QueryKind};
use crate::task_ctx::mark;
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, Layer};
use std::fmt;

struct SqlVisitor { sql: Option<String> }
impl SqlVisitor {
    fn new() -> Self { Self { sql: None } }
}
impl tracing::field::Visit for SqlVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "db.statement" | "statement" | "message" => { self.sql = Some(value.to_string()); }
            _ => {}
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        match field.name() {
            "db.statement" | "statement" | "message" => { self.sql = Some(format!("{value:?}")); }
            _ => {}
        }
    }
}

pub struct SqlxQueryLayer;
impl SqlxQueryLayer { pub fn new() -> Self { Self } }

impl<S> Layer<S> for SqlxQueryLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let target = meta.target();
        if !target.starts_with("sqlx::query") { return; }

        let mut vis = SqlVisitor::new();
        event.record(&mut vis);

        let key = vis.sql.map(|s| normalize_sql(&s)).unwrap_or_else(|| target.to_string());
        mark(QueryKind::Sql, &key);
    }
}
