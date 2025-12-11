// /Users/snm/Equicom/workspace/NS/crates/moniof/src/config/global.rs

use once_cell::sync::OnceCell;
use parking_lot::RwLock;

// -------------------------------------------------------
// Global Config Struct
// -------------------------------------------------------
#[derive(Clone, Default)]
pub struct MoniOFGlobalConfig {
    /// Log each DB command start/finish at DEBUG level
    pub log_each_db_event: bool,

    /// Slow single DB command threshold (ms) => warn (+ optional Slack)
    pub slow_db_threshold_ms: Option<u64>,

    /// Suspiciously low DB command threshold (ms)
    pub low_db_threshold_ms: Option<u64>,

    /// Slack webhook URL for alerts (optional)
    pub slack_webhook: Option<String>,
}

static GLOBAL: OnceCell<RwLock<MoniOFGlobalConfig>> = OnceCell::new();


// -------------------------------------------------------
// INITIATE (GLOBAL INIT + TRACING SETUP)
// -------------------------------------------------------
pub fn initiate(cfg: MoniOFGlobalConfig) {
    use tracing_subscriber::{fmt, EnvFilter, prelude::*};

    // Build RUST_LOG + moniof fallback filter
    let base = EnvFilter::from_default_env();

    let filter = base
        .add_directive("moniof=debug".parse().unwrap_or_else(|_| "debug".parse().unwrap()))
        .add_directive("moniof::mongo=debug".parse().unwrap_or_else(|_| "debug".parse().unwrap()))
        .add_directive("moniof::sql=debug".parse().unwrap_or_else(|_| "debug".parse().unwrap()))
        .add_directive("moniof::of=debug".parse().unwrap_or_else(|_| "debug".parse().unwrap()))
        .add_directive("sqlx=info".parse().unwrap_or_else(|_| "info".parse().unwrap())); // SQLx internal logs

    let fmt_layer = fmt::layer().with_target(true);

    #[cfg(feature = "sqlx")]
    {
        use crate::instrumentation::sql_events::MOFSqlEvents;

        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(MOFSqlEvents::new()); // ADD SQL INSTRUMENTATION HERE

        let _ = subscriber.try_init();
    }

    #[cfg(not(feature = "sqlx"))]
    {
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer);

        let _ = subscriber.try_init();
    }

    let cell = GLOBAL.get_or_init(|| RwLock::new(MoniOFGlobalConfig::default()));
    *cell.write() = cfg;

    tracing::info!(target = "moniof", "moniof global initiated (SQL logging enabled)");
}

// -------------------------------------------------------
// GETTER
// -------------------------------------------------------
pub fn global() -> MoniOFGlobalConfig {
    GLOBAL
        .get()
        .map(|g| g.read().clone())
        .unwrap_or_default()
}
