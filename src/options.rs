use once_cell::sync::OnceCell;
use parking_lot::RwLock;

#[derive(Clone, Default)]
pub struct MoniOFGlobalConfig {
    /// Log each DB command start/finish at DEBUG level
    pub log_each_db_event: bool,
    /// Slow single DB command threshold (ms) => warn (+ optional Slack)
    pub slow_db_threshold_ms: Option<u64>,
    pub low_db_threshold_ms: Option<u64>,
    /// Slack webhook for alerts (optional)
    pub slack_webhook: Option<String>,
}

static GLOBAL: OnceCell<RwLock<MoniOFGlobalConfig>> = OnceCell::new();

pub fn init_global(cfg: MoniOFGlobalConfig) {
    let cell = GLOBAL.get_or_init(|| RwLock::new(MoniOFGlobalConfig::default()));
    *cell.write() = cfg;
}

pub fn global() -> MoniOFGlobalConfig {
    GLOBAL
        .get()
        .map(|g| g.read().clone())
        .unwrap_or_default()
}
