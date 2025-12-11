#![cfg(feature = "mongodb")]

use mongodb::event::command::{
    CommandEventHandler,
    CommandStartedEvent,
    CommandSucceededEvent,
    CommandFailedEvent,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::time::Instant;

use crate::config::global;
use crate::observability::prom;
use crate::core::stats::QueryKind;
use crate::core::task_ctx::{mark, mark_latency};
use crate::observability::slack;

/// We track mongo commands by (connection, request_id)
/// and store (started_at, collection, op) as value.
static INFLIGHT: Lazy<DashMap<(String, i32), (Instant, String, String)>> =
    Lazy::new(DashMap::new);

// Build a stable key for the inflight map
fn inflight_key(connection_dbg: &str, request_id: i32) -> (String, i32) {
    (connection_dbg.to_string(), request_id)
}

/// Extract a reasonable (collection, op) from the started event.
/// Fallbacks are cheap and good enough for observability labels.
fn extract_collection_op(event: &CommandStartedEvent) -> (String, String) {
    // Try `collection` field first (not always present).
    let collection = event
        .command
        .get_str("collection")
        .ok()
        .map(|s| s.to_string())
        // fallback to db name if no explicit collection
        .unwrap_or_else(|| event.db.clone());

    let op = event.command_name.to_lowercase();
    (collection, op)
}

/// Main MongoDB CommandEventHandler used by moniof.
///
/// Attach this handler to ClientOptions::command_event_handler to let moniof:
/// - count Mongo operations,
/// - measure per-command latency,
/// - update per-request DB totals,
/// - emit logs & Slack alerts for slow/failed ops.
#[derive(Default, Debug)]
pub struct MOFMongoEvents;

impl CommandEventHandler for MOFMongoEvents {
    fn handle_command_started_event(&self, event: CommandStartedEvent) {
        let cfg = global();

        let connection_dbg = format!("{:?}", event.connection);
        let key_inflight = inflight_key(&connection_dbg, event.request_id);
        let started_at = Instant::now();

        let (collection, op) = extract_collection_op(&event);
        let logical_key = format!("{}/{}", collection, op);

        // Track this command in our inflight map
        INFLIGHT.insert(key_inflight, (started_at, collection.clone(), op.clone()));

        // Count query immediately
        mark(QueryKind::Mongo, &logical_key);

        if cfg.log_each_db_event {
            tracing::debug!(
                target = "MoniOF::mongo",
                db = %event.db,
                command = %event.command_name,
                key = %logical_key,
                "mongo started"
            );
        }
    }

    fn handle_command_succeeded_event(&self, event: CommandSucceededEvent) {
        let cfg = global();

        let connection_dbg = format!("{:?}", event.connection);
        let key_inflight = inflight_key(&connection_dbg, event.request_id);

        let (started_at, collection, op) = INFLIGHT
            .remove(&key_inflight)
            .map(|(_, v)| v)
            .unwrap_or_else(|| (Instant::now(), "unknown".to_string(), event.command_name.to_lowercase()));

        let ms = started_at.elapsed().as_millis();
        let logical_key = format!("{}/{}", collection, op);

        // Record latency
        mark_latency(QueryKind::Mongo, &logical_key, ms);

        // Prometheus observation
        prom::observe_mongo_cmd(&collection, &op, (ms as f64) / 1000.0);

        if cfg.log_each_db_event {
            tracing::info!(
                target = "MoniOF::mongo",
                key = %logical_key,
                latency_ms = %ms,
                "mongo ok"
            );
        }

        if let Some(th) = cfg.slow_db_threshold_ms {
            if ms >= th as u128 {
                tracing::warn!(
                    target = "MoniOF::mongo",
                    key = %logical_key,
                    latency_ms = %ms,
                    threshold_ms = th,
                    "slow mongo command"
                );
                if let Some(ref hook) = cfg.slack_webhook {
                    let text = format!(
                        "🐢 *Slow MongoDB command*\n• `key`: `{}`\n• `latency`: {} ms",
                        logical_key, ms
                    );
                    tokio::spawn(slack::notify(Some(hook.clone()), text));
                }
            }
        }

        if let Some(low) = cfg.low_db_threshold_ms {
            if ms <= low as u128 {
                tracing::debug!(
                    target = "MoniOF::mongo",
                    key = %logical_key,
                    latency_ms = %ms,
                    threshold_ms = low,
                    "very fast mongo command (check instrumentation/cache?)"
                );
            }
        }
    }

    fn handle_command_failed_event(&self, event: CommandFailedEvent) {
        let cfg = global();

        let connection_dbg = format!("{:?}", event.connection);
        let key_inflight = inflight_key(&connection_dbg, event.request_id);

        let (started_at, collection, op) = INFLIGHT
            .remove(&key_inflight)
            .map(|(_, v)| v)
            .unwrap_or_else(|| (Instant::now(), "unknown".to_string(), event.command_name.to_lowercase()));

        let ms = started_at.elapsed().as_millis();
        let logical_key = format!("{}/{}", collection, op);

        mark_latency(QueryKind::Mongo, &logical_key, ms);
        prom::observe_mongo_cmd(&collection, &op, (ms as f64) / 1000.0);

        tracing::warn!(
            target = "MoniOF::mongo",
            key = %logical_key,
            latency_ms = %ms,
            "mongo failed"
        );

        if let Some(ref hook) = cfg.slack_webhook {
            let text = format!(
                "❌ *MongoDB command failed*\n• `key`: `{}`\n• `latency`: {} ms",
                logical_key, ms
            );
            tokio::spawn(slack::notify(Some(hook.clone()), text));
        }
    }
}
