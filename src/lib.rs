// /Users/snm/Equicom/workspace/NS/crates/moniof/src/lib.rs
pub mod config;
pub mod core;
pub mod instrumentation;
pub mod observability;
pub mod services;

// Keep public API roughly compatible:
pub use config::{MoniOFGlobalConfig, initiate, global};
pub use config::MoniOFConfig;
pub use services::http::MoniOF;


pub use observability::prom;


#[cfg(feature = "mongodb")]
pub use instrumentation::mongo_events::MOFMongoEvents;


#[cfg(feature = "sqlx")]
pub use instrumentation::sql_events::MOFSqlEvents;
