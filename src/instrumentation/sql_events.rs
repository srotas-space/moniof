// src/instrumentation/sql_events.rs
#![cfg(feature = "sqlx")]

use crate::core::stats::{normalize_sql, QueryKind};
use crate::core::task_ctx::{mark, mark_latency};

use std::fmt;
use std::time::Instant;

use tracing::{span::Attributes, Event, Id, Subscriber};
use tracing_subscriber::{layer::Context, Layer};

/// Internal storage for SQL spans.
struct SqlSpanData {
    key: String,
    started_at: Instant,
}

/// Visitor that extracts SQL from span attributes.
struct SqlVisitor {
    sql: Option<String>,
}

impl SqlVisitor {
    fn new() -> Self {
        Self { sql: None }
    }
}

impl tracing::field::Visit for SqlVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "db.statement" | "statement" | "message" => {
                self.sql = Some(value.to_string());
            }
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if self.sql.is_none() && field.name() == "message" {
            self.sql = Some(format!("{value:?}"));
        }
    }
}

/// SQLx instrumentation layer for moniof
pub struct MOFSqlEvents;

impl MOFSqlEvents {
    pub fn new() -> Self {
        MOFSqlEvents
    }
}

impl<S> Layer<S> for MOFSqlEvents
where
    S: Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    // When span is created
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(s) => s,
            None => return,
        };

        let target = span.metadata().target();

        if !target.starts_with("sqlx::query") {
            return;
        }

        let mut vis = SqlVisitor::new();
        attrs.record(&mut vis);

        let raw_sql = vis.sql.unwrap_or_else(|| target.to_string());
        let key = normalize_sql(&raw_sql);

        // Store for finalization
        span.extensions_mut().insert(SqlSpanData {
            key: key.clone(),
            started_at: Instant::now(),
        });

        tracing::debug!(
            target = "MoniOF::sql",
            query = %raw_sql,
            normalized = %key,
            "SQL started"
        );
    }

    // When span completes successfully
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let span = match ctx.span(&id) {
            Some(s) => s,
            None => return,
        };

        let mut exts = span.extensions_mut();

        if let Some(data) = exts.remove::<SqlSpanData>() {
            let ms = data.started_at.elapsed().as_millis();
            let key = data.key.clone();

            mark(QueryKind::Sql, &key);
            mark_latency(QueryKind::Sql, &key, ms);

            tracing::info!(
                target = "MoniOF::sql",
                key = %key,
                latency_ms = %ms,
                "SQL completed"
            );
        }
    }

    // Handle SQL event-only mode (fallback)
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let target = event.metadata().target();
        if !target.starts_with("sqlx::query") {
            return;
        }

        let mut vis = SqlVisitor::new();
        event.record(&mut vis);

        let raw_sql = vis.sql.unwrap_or_else(|| target.to_string());
        let key = normalize_sql(&raw_sql);

        mark(QueryKind::Sql, &key);

        tracing::debug!(
            target = "MoniOF::sql",
            query = %raw_sql,
            normalized = %key,
            "SQL event-only mode"
        );
    }
}
