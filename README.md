# moniof — Monitor Over Fetch

[![Crates.io](https://img.shields.io/crates/v/moniof)](https://crates.io/crates/moniof)
[![Documentation](https://docs.rs/moniof/badge.svg)](https://docs.rs/moniof)
[![License](https://img.shields.io/crates/l/moniof)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/moniof)](https://crates.io/crates/moniof)
[![Recent Downloads](https://img.shields.io/crates/dr/moniof)](https://crates.io/crates/moniof)

![MoniOF](https://srotas-suite-space.s3.ap-south-1.amazonaws.com/snmoniof.png)


**moniof** (_Monitor Over Fetch_) is an Actix Web middleware + instrumentation crate to:

- detect **N+1 / over-fetch** patterns  
- track **per-request DB latency**  
- expose **Prometheus metrics**  
- send **Slack alerts**  
- work with **MongoDB** and **SQLx** (Postgres / MySQL / SQLite)

Inspired by Ruby's **bullet** gem — but built for **Rust + Actix**.

---

## ✨ Features

- 🧱 Actix middleware (`MoniOF`)
- 🕵️ N+1 & Over-Fetch detection
- 📡 MongoDB instrumentation (command events)
- 🧮 SQLx instrumentation (via tracing spans)
- 📊 Prometheus metrics
- 🔔 Slack alerts for slow DB calls
- 🧾 Auto response headers:
  - `x-moniof-total`
  - `x-moniof-db-total-ms`
  - `x-moniof-elapsed-ms`
  - `x-moniof-slowest-key`
  - `x-moniof-n-plus-one-key`

---

## 🚀 Installation

Add to your app's Cargo.toml:

```toml
[dependencies]
moniof = { version = "0.1.0", features = ["mongodb", "sqlx"] }
actix-web = "4"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "registry"] }

mongodb = "2"
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio-rustls"] }
```

---

## 🧩 Public API

```rust
pub use config::{MoniOFGlobalConfig, initiate, global};
pub use config::MoniOFConfig;
pub use services::http::MoniOF;
pub use observability::prom;

#[cfg(feature = "mongodb")]
pub use instrumentation::mongo_events::MOFMongoEvents;

#[cfg(feature = "sqlx")]
pub use instrumentation::sql_events::MOFSqlEvents;
```

---

## 🌐 Step 1 — Initialize moniof globally

Call **once** in `main()`:

```rust
use moniof::{MoniOFGlobalConfig, initiate as moniof_initiate};

fn main() {
    moniof_initiate(MoniOFGlobalConfig {
        slack_webhook: None,
        slow_db_threshold_ms: Some(0),
        low_db_threshold_ms: Some(0),
        log_each_db_event: true,
        ..Default::default()
    });

    // Start Actix...
}
```

This installs:

- tracing subscriber  
- SQLx instrumentation (if feature enabled)  
- log filter for moniof  

---

## 🌍 Step 2 — Add Actix Middleware

```rust
use moniof::MoniOF;

HttpServer::new(|| {
    App::new()
        .wrap(MoniOF::new())
})
```

Now each request produces:

- DB stats
- detection of N+1
- enriched response headers

---

## 🍃 MongoDB Integration

Attach moniof's MongoDB handler:

```rust
use moniof::MOFMongoEvents;
use std::sync::Arc;
use mongodb::{Client, options::ClientOptions};

let mut opts = ClientOptions::parse(&mongo_uri).await?;
opts.command_event_handler = Some(Arc::new(MOFMongoEvents::default()));

let client = Client::with_options(opts)?;
let db = client.database("mydb");
```

Every `find`, `insert`, `update` is tracked.

---

## 🧮 SQLx Integration

If your crate enables:

```toml
features = ["sqlx"]
```

Then **SQLx logs are automatically hooked**.

Use SQLx normally:

```rust
let rows = sqlx::query!("SELECT id FROM users")
    .fetch_all(pool)
    .await?;
```

moniof will record:

- query count  
- query latency  
- over-fetch patterns  

and log:

```
moniof::sql: SQL completed key="select from users" latency_ms=2
```

---

## 📈 Prometheus Metrics

Expose `/metrics`:

```rust
use moniof::prom;

HttpServer::new(|| {
    App::new()
        .service(prom::metrics())
})
```

Example metrics:

```
moniof_http_requests_total
moniof_http_request_duration_seconds
moniof_db_total_latency_seconds
moniof_mongo_command_duration_seconds
```

---

## 🔔 Slack Alerts

Enabled when `slack_webhook` is set:

```rust
slack_webhook: Some("https://hooks.slack.com/...".to_string())
```

Alert types:

- Slow DB commands  
- Mongo/SQLx failures  
- N+1 detection  

---

## 🧪 Example Response Headers

```
x-moniof-total: 5
x-moniof-db-total-ms: 12
x-moniof-elapsed-ms: 18
x-moniof-slowest-key: users/find
x-moniof-n-plus-one-key: users/find
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.


## 🙏 Acknowledgments

- Built with [Actix Web](https://actix.rs/) - Fast, powerful web framework


---

Made with ❤️ by the [Srotas Space] (https://srotas.space/open-source)

---

[![GitHub stars](https://img.shields.io/github/stars/srotas-space/moniof?style=social)](https://github.com/srotas-space/moniof)
[![LinkedIn Follow](https://img.shields.io/badge/LinkedIn-Follow-blue?style=social&logo=linkedin)](https://www.linkedin.com/company/srotas-space)


## Support

- **Documentation**: [docs.rs/moniof](https://docs.rs/moniof)
- **Issues**: [GitHub Issues](https://github.com/srotas-space/moniof/issues)
- **Discussions**: [GitHub Discussions](https://github.com/srotas-space/moniof/discussions)

