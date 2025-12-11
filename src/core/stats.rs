use ahash::AHashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind { Mongo, Sql, Other }

#[derive(Debug)]
pub struct QueryStats {
    pub started_at: OffsetDateTime,
    pub total: usize,
    pub per_key: AHashMap<String, usize>,

    pub total_db_latency_ms: u128,
    pub per_key_latency_ms: AHashMap<String, u128>,
    pub per_key_max_latency_ms: AHashMap<String, u128>,
}

impl QueryStats {
    pub fn new() -> Self {
        Self {
            started_at: OffsetDateTime::now_utc(),
            total: 0,
            per_key: AHashMap::new(),
            total_db_latency_ms: 0,
            per_key_latency_ms: AHashMap::new(),
            per_key_max_latency_ms: AHashMap::new(),
        }
    }

    pub fn record(&mut self, key: &str) {
        self.total += 1;
        *self.per_key.entry(key.to_string()).or_insert(0) += 1;
    }

    pub fn record_latency(&mut self, key: &str, ms: u128) {
        self.total_db_latency_ms += ms;
        *self.per_key_latency_ms.entry(key.to_string()).or_insert(0) += ms;
        let e = self.per_key_max_latency_ms.entry(key.to_string()).or_insert(0);
        if ms > *e { *e = ms; }
    }

    pub fn elapsed(&self) -> Duration {
        OffsetDateTime::now_utc() - self.started_at
    }
}

#[derive(Clone)]
pub struct QueryStatsHandle(pub Arc<Mutex<QueryStats>>);
impl QueryStatsHandle {
    pub fn new() -> Self { Self(Arc::new(Mutex::new(QueryStats::new()))) }
}

// SQL normalization helper (used by sqlx layer)
pub fn normalize_sql(sql: &str) -> String {
    let mut reduced = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if reduced.len() > 200 { reduced.truncate(200); }
    reduced.to_lowercase()
}
