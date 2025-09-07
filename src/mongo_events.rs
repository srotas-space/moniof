#![cfg(feature = "mongodb")]

use mongodb::event::command::{
    CommandEventHandler, CommandFailedEvent, CommandStartedEvent, CommandSucceededEvent,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::time::Instant;

use crate::options::global;
use crate::prom;
use crate::stats::QueryKind;
use crate::task_ctx::{mark, mark_latency};
use crate::slack;

// Track command start times keyed by (connection + request_id)
static INFLIGHT: Lazy<DashMap<String, (Instant, String)>> = Lazy::new(|| DashMap::new());

fn parse_key(ev: &CommandStartedEvent) -> (String, String, String) {
    let op = ev.command_name.as_str().to_string(); // e.g. "find"
    let coll = ev.command.get_str(&op).unwrap_or("<unknown>").to_string();
    let conn = format!("{:?}", ev.connection);
    let id = format!("{conn}#{}", ev.request_id);
    (id, coll, op)
}

#[derive(Default)]
pub struct MoniOFMongoEventHandler;

impl CommandEventHandler for MoniOFMongoEventHandler {
    fn handle_command_started_event(&self, ev: CommandStartedEvent) {
        let cfg = global();
        let (id, coll, op) = parse_key(&ev);
        let key = format!("{}/{}", coll, op);
        INFLIGHT.insert(id, (Instant::now(), key.clone()));
        mark(QueryKind::Mongo, &key);

        if cfg.log_each_db_event {
            tracing::info!("---------------------------------------------------------------------");
            tracing::info!("-----------------------------");
            tracing::debug!(target="moniof::mongo", collection=%coll, op=%op, "mongo started");
            tracing::info!("-----------------------------");
            tracing::info!("---------------------------------------------------------------------");
        }
    }

    fn handle_command_succeeded_event(&self, ev: CommandSucceededEvent) {
        let cfg = global();
        let conn = format!("{:?}", ev.connection);
        let id = format!("{conn}#{}", ev.request_id);

        if let Some((start, key)) = INFLIGHT.remove(&id).map(|e| e.1) {
            let ms = start.elapsed().as_millis();
            // split key into collection/op
            let (coll, op) = key.split_once('/').unwrap_or((&key[..], "unknown"));
            prom::observe_mongo_cmd(coll, op, (ms as f64) / 1000.0);

            mark_latency(QueryKind::Mongo, &key, ms);

            // SLOW alert
            if let Some(th) = cfg.slow_db_threshold_ms {
                if ms as u64 >= th {
                    tracing::info!("---------------------------------------------------------------------");
                    tracing::info!("-------------------------------------");
                    tracing::info!("-----------------");
                    tracing::info!("----MoniOF----");
                    tracing::warn!(target="moniof::mongo", key=%key, latency_ms=%ms, "slow mongo command");
                    tracing::info!("----MoniOF----");
                    tracing::info!("-----------------");
                    tracing::info!("-------------------------------------");
                    tracing::info!("---------------------------------------------------------------------");
                    if let Some(ref hook) = cfg.slack_webhook {
                        let text = format!("⚠️ *Slow MongoDB command*\n• `key`: `{}`\n• `latency`: {} ms", key, ms);
                        tokio::spawn(slack::notify(hook.clone(), text));
                    }
                }
            }
            // LOW alert (if configured)
            if let Some(low) = cfg.low_db_threshold_ms {
                if ms as u64 <= low {
                    tracing::info!("---------------------------------------------------------------------");
                    tracing::info!("-------------------------------------");
                    tracing::info!("-----------------");
                    tracing::info!("----MoniOF----");
                    tracing::warn!(target="moniof::mongo", key=%key, latency_ms=%ms, threshold=low, "unusually LOW mongo command latency");
                    tracing::info!("----MoniOF----");
                    tracing::info!("-----------------");
                    tracing::info!("-------------------------------------");
                    tracing::info!("---------------------------------------------------------------------");
                    if let Some(ref hook) = cfg.slack_webhook {
                        let text = format!("ℹ️ *Low-latency MongoDB command*\n• `key`: `{}`\n• `latency`: {} ms (<= {} ms)", key, ms, low);
                        tokio::spawn(slack::notify(hook.clone(), text));
                    }
                }
            }

            if cfg.low_db_threshold_ms.is_none() && cfg.slow_db_threshold_ms.is_none() && cfg.log_each_db_event {
                tracing::debug!(target="moniof::mongo", key=%key, latency_ms=%ms, "mongo ok");
            }
        }
    }

    fn handle_command_failed_event(&self, ev: CommandFailedEvent) {
        let cfg = global();
        let conn = format!("{:?}", ev.connection);
        let id = format!("{conn}#{}", ev.request_id);

        if let Some((start, key)) = INFLIGHT.remove(&id).map(|e| e.1) {
            let ms = start.elapsed().as_millis();
            mark_latency(QueryKind::Mongo, &key, ms);
            tracing::info!("---------------------------------------------------------------------");
            tracing::info!("-------------------------------------");
            tracing::info!("-----------------");
            tracing::info!("----MoniOF----");
            tracing::warn!(target="moniof::mongo", key=%key, latency_ms=%ms, "mongo failed");
            tracing::info!("----MoniOF----");
            tracing::info!("-----------------");
            tracing::info!("-------------------------------------");
            tracing::info!("---------------------------------------------------------------------");
            if let Some(ref hook) = cfg.slack_webhook {
                let text = format!("❌ *MongoDB command failed*\n• `key`: `{}`\n• `latency`: {} ms", key, ms);
                tokio::spawn(slack::notify(hook.clone(), text));
            }
        }
    }
}
