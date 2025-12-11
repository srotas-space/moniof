// /Users/snm/Equicom/workspace/NS/crates/moniof/src/services/http.rs

use crate::config::{MoniOFConfig, global};
use crate::core::stats::QueryStatsHandle;
use crate::core::task_ctx::MONIOF_HANDLE;
use crate::observability::{prom, slack, of};

use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use std::{
    rc::Rc,
    task::{Context, Poll},
    time::Instant,
};
use tracing;

pub struct MoniOF {
    cfg: MoniOFConfig,
}

impl MoniOF {
    pub fn new() -> Self {
        Self {
            cfg: MoniOFConfig::default(),
        }
    }

    pub fn with_config(cfg: MoniOFConfig) -> Self {
        Self { cfg }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MoniOF
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = MoniOFMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        prom::init_prometheus();
        ready(Ok(MoniOFMiddleware {
            service: Rc::new(service),
            cfg: self.cfg.clone(),
        }))
    }
}

pub struct MoniOFMiddleware<S> {
    pub(crate) service: Rc<S>,
    pub(crate) cfg: MoniOFConfig,
}

impl<S, B> Service<ServiceRequest> for MoniOFMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();
        let cfg = self.cfg.clone();

        // capture method for metrics before move
        let method = req.method().as_str().to_string();
        prom::inc_inflight();
        let req_start = Instant::now();

        Box::pin(async move {
            // per-request query stats handle
            let handle = QueryStatsHandle::new();
            let handle_for_read = handle.clone();

            // install task-local context so mark/mark_latency work
            let mut res = MONIOF_HANDLE
                .scope(handle, async move {
                    // inner service call returns Result<ServiceResponse<B>, Error>
                    svc.call(req).await
                })
                .await?; // now `?` applies to Result<_, Error>

            let req_duration_s = req_start.elapsed().as_secs_f64();
            prom::dec_inflight();

            // --------------------------
            // Read stats for this request
            // --------------------------
            let stats = handle_for_read.0.lock();
            let total = stats.total;
            let elapsed_ms = stats.elapsed().whole_milliseconds();
            let db_total_ms = stats.total_db_latency_ms;

            // most-repeated key (by count)
            let mut worst_count: Option<(&String, &usize)> = None;
            for (k, v) in &stats.per_key {
                if worst_count.map(|(_, c)| v > c).unwrap_or(true) {
                    worst_count = Some((k, v));
                }
            }

            // slowest key (by max latency)
            let mut slowest_key: Option<(&String, &u128)> = None;
            for (k, v) in &stats.per_key_max_latency_ms {
                if slowest_key.map(|(_, m)| v > m).unwrap_or(true) {
                    slowest_key = Some((k, v));
                }
            }

            // OF-style / OF-like N+1 suspects (via `of` module)
            let n_plus_one_suspects = of::find_suspects(&stats, &cfg);

            let status = res.status().as_u16();
            prom::observe_request(
                &method,
                status,
                req_duration_s,
                (db_total_ms as f64) / 1000.0,
            );

            // --------------------------
            // Response headers
            // --------------------------
            if cfg.add_response_headers {
                let headers = res.headers_mut();
                let mut put = |name: &'static str, val: String| {
                    let name = HeaderName::from_static(name);
                    if let Ok(hv) = HeaderValue::from_str(&val) {
                        headers.insert(name, hv);
                    }
                };

                put("x-moniof-total", total.to_string());
                put("x-moniof-elapsed-ms", elapsed_ms.to_string());
                put("x-moniof-db-total-ms", db_total_ms.to_string());

                if let Some((k, v)) = slowest_key.as_ref() {
                    put("x-moniof-slowest-key", (*k).to_string());
                    put("x-moniof-slowest-latency-ms", (**v).to_string());
                }

                if cfg.of_mode && !n_plus_one_suspects.is_empty() {
                    if let Some(top) = n_plus_one_suspects.first() {
                        put("x-moniof-n-plus-one-key", top.key.clone());
                        put("x-moniof-n-plus-one-count", top.count.to_string());
                        put(
                            "x-moniof-n-plus-one-total-ms",
                            top.total_latency_ms.to_string(),
                        );
                    }
                }
            }

            // --------------------------
            // Warnings + Slack alerts (OF-style)
            // --------------------------
            if cfg.log_warnings {
                let mut alerted = false;

                // High total query count (possible N+1 overall)
                if total > cfg.max_total {
                    alerted = true;
                    tracing::warn!(
                        target = "moniof",
                        total,
                        max_total = cfg.max_total,
                        elapsed_ms,
                        db_total_ms,
                        "High DB query count (possible N+1)"
                    );
                }

                // Worst key by count (single key repeated a lot)
                if let Some((k, v)) = worst_count {
                    if *v > cfg.max_same_key {
                        alerted = true;
                        tracing::warn!(
                            target = "moniof",
                            key = %k,
                            count = %v,
                            max_same_key = cfg.max_same_key,
                            "Repeated same DB key (N+1 likely)"
                        );
                    }
                }

                // High cumulative DB latency
                if let Some(th) = cfg.warn_total_db_latency_ms {
                    if db_total_ms >= th {
                        alerted = true;
                        tracing::warn!(
                            target = "moniof",
                            db_total_ms,
                            threshold = th,
                            "High cumulative DB latency in request"
                        );
                    }
                }

                // Suspiciously *low* DB latency (instrumentation/cache sanity)
                if let Some(low) = cfg.warn_low_total_db_latency_ms {
                    if total > 0 && db_total_ms <= low {
                        alerted = true;
                        tracing::warn!(
                            target = "moniof",
                            total,
                            db_total_ms,
                            threshold = low,
                            "Suspiciously LOW cumulative DB latency (check instrumentation or cache?)"
                        );
                    }
                }

                // Explicit N+1 suspects (OF-style)
                if cfg.of_mode && !n_plus_one_suspects.is_empty() {
                    alerted = true;
                    for s in &n_plus_one_suspects {
                        tracing::warn!(
                            target = "moniof::of",
                            key = %s.key,
                            count = %s.count,
                            total_latency_ms = %s.total_latency_ms,
                            "Possible N+1 detected (OF-like)"
                        );
                    }
                }

                // Send Slack if any alert fired
                if alerted {
                    let g = global();
                    if let Some(hook) = g.slack_webhook {
                        let mut lines = vec![
                            "⚠️ *moniOF alert*".to_string(),
                            format!("• status: {}", status),
                            format!("• method: {}", method),
                            format!("• total queries: {}", total),
                            format!("• req elapsed: {:.3}s", req_duration_s),
                            format!("• db total latency: {} ms", db_total_ms),
                        ];
                        if let Some((k, v)) = slowest_key.as_ref() {
                            lines.push(format!("• slowest key: `{}` ({} ms)", k, v));
                        }
                        if let Some((k, v)) = worst_count.as_ref() {
                            lines.push(format!("• worst key (count): `{}` ×{}", k, v));
                        }
                        if cfg.of_mode && !n_plus_one_suspects.is_empty() {
                            lines.push("• *N+1 suspects* (OF-like):".to_string());
                            for s in &n_plus_one_suspects {
                                lines.push(format!(
                                    "    ↳ `{}` — {}× ({} ms total)",
                                    s.key, s.count, s.total_latency_ms
                                ));
                            }
                        }
                        tokio::spawn(slack::notify(hook, lines.join("\n")));
                    }
                }
            }

            Ok(res)
        })
    }
}
