#[derive(Clone, Debug)]
pub struct MoniOFConfig {
    pub max_total: usize,
    pub max_same_key: usize,
    pub add_response_headers: bool,
    pub log_warnings: bool,
    /// Warn when *cumulative* DB latency exceeds this (ms)
    pub warn_total_db_latency_ms: Option<u128>,
    /// Alert when *cumulative* DB latency is unusually low (ms) but queries > 0
    pub warn_low_total_db_latency_ms: Option<u128>,

    /// OF-style N+1 detection
    pub of_mode: bool,
    /// Minimum times a key must repeat in a request to be considered N+1.
    pub n_plus_one_min_count: usize,
    /// Optional minimum total latency for that key to be considered N+1.
    pub n_plus_one_min_total_ms: Option<u128>,
}

impl Default for MoniOFConfig {
    fn default() -> Self {
        Self {
            max_total: 60,
            max_same_key: 20,
            add_response_headers: true,
            log_warnings: true,
            warn_total_db_latency_ms: None,
            warn_low_total_db_latency_ms: None,

            of_mode: true,
            n_plus_one_min_count: 5,
            n_plus_one_min_total_ms: Some(5),
        }
    }
}
