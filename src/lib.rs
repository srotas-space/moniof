pub mod middleware;
pub mod stats;
mod task_ctx;

pub mod options;
pub use options::{MoniOFGlobalConfig, init_global, global};

pub use middleware::{MoniOF, MoniOFConfig};

#[cfg(feature = "mongodb")]
pub mod mongo_events;

// Optional wrapper mode (not required for zero-refactor event-hook setup)
#[cfg(feature = "mongodb")]
pub mod mongo;

// Optional (future) SQLx layer
#[cfg(feature = "sqlx")]
pub mod sqlx_layer;

// Prometheus metrics
pub mod prom;


mod slack;
