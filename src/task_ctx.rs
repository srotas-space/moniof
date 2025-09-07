use crate::stats::{QueryKind, QueryStatsHandle};
use tokio::task_local;

task_local! {
    pub static MONIOF_HANDLE: QueryStatsHandle;
}

pub fn mark(kind: QueryKind, key: &str) {
    let _ = MONIOF_HANDLE.try_with(|h| {
        let mut stats = h.0.lock();
        stats.record(&format!("{}/{}", match kind {
            QueryKind::Mongo => "mongo",
            QueryKind::Sql   => "sql",
            QueryKind::Other => "other",
        }, key));
    });
}

pub fn mark_latency(kind: QueryKind, key: &str, ms: u128) {
    let _ = MONIOF_HANDLE.try_with(|h| {
        let mut stats = h.0.lock();
        stats.record_latency(&format!("{}/{}", match kind {
            QueryKind::Mongo => "mongo",
            QueryKind::Sql   => "sql",
            QueryKind::Other => "other",
        }, key), ms);
    });
}
