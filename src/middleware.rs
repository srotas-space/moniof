use crate::options::global;
use crate::prom;
use crate::slack;
use crate::stats::QueryStatsHandle;
use crate::task_ctx::MONIOF_HANDLE;

use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use std::{rc::Rc, task::{Context, Poll}, time::Instant};
use tracing;

#[derive(Clone, Debug)]
pub struct MoniOFConfig {
    pub max_total: usize,
    pub max_same_key: usize,
    pub add_response_headers: bool,
    pub log_warnings: bool,
    /// Warn when *cumulative* DB latency in a request exceeds this (ms)
    pub warn_total_db_latency_ms: Option<u128>,
    /// NEW: Alert when *cumulative* DB latency is unusually low (ms) but queries > 0
    pub warn_low_total_db_latency_ms: Option<u128>,
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
        }
    }
}

pub struct MoniOF { cfg: MoniOFConfig }
impl MoniOF {
    pub fn new() -> Self { Self { cfg: MoniOFConfig::default() } }
    pub fn with_config(cfg: MoniOFConfig) -> Self { Self { cfg } }
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
        // ensure prometheus is ready
        prom::init_prometheus();
        ready(Ok(MoniOFMiddleware { service: Rc::new(service), cfg: self.cfg.clone() }))
    }
}

pub struct MoniOFMiddleware<S> {
    service: Rc<S>,
    cfg: MoniOFConfig,
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

        // capture info before move
        let method = req.method().as_str().to_string();
        prom::inc_inflight();
        let req_start = Instant::now();

        Box::pin(async move {
            let handle = QueryStatsHandle::new();
            let handle_for_read = handle.clone();

            let mut res = match MONIOF_HANDLE.scope(handle, async move { svc.call(req).await }).await {
                Ok(r) => r,
                Err(e) => {
                    prom::dec_inflight();
                    return Err(e);
                }
            };

            let req_duration_s = req_start.elapsed().as_secs_f64();
            prom::dec_inflight();

            let stats = handle_for_read.0.lock();
            let total = stats.total;
            let elapsed_ms = stats.elapsed().whole_milliseconds();
            let db_total_ms = stats.total_db_latency_ms;

            // most-repeated key (by count)
            let mut worst_count: Option<(&String, &usize)> = None;
            for (k, v) in &stats.per_key {
                if worst_count.map(|(_, c)| v > c).unwrap_or(true) { worst_count = Some((k, v)); }
            }
            // slowest key (by max latency)
            let mut slowest_key: Option<(&String, &u128)> = None;
            for (k, v) in &stats.per_key_max_latency_ms {
                if slowest_key.map(|(_, m)| v > m).unwrap_or(true) { slowest_key = Some((k, v)); }
            }

            // Prometheus observations
            let status = res.status().as_u16();
            prom::observe_request(&method, status, req_duration_s, (db_total_ms as f64) / 1000.0);

            if cfg.add_response_headers {
                let headers = res.headers_mut();
                let mut put = |name: &'static str, val: String| {
                    let name = HeaderName::from_static(name);
                    if let Ok(hv) = HeaderValue::from_str(&val) { headers.insert(name, hv); }
                };

                put("x-moniof-total", total.to_string());
                put("x-moniof-elapsed-ms", elapsed_ms.to_string());
                put("x-moniof-db-total-latency-ms", db_total_ms.to_string());
                if let Some((k, v)) = worst_count.as_ref() {
                    put("x-moniof-worst-key", (*k).to_string());
                    put("x-moniof-worst-count", (**v).to_string());
                }
                if let Some((k, v)) = slowest_key.as_ref() {
                    put("x-moniof-slowest-key", (*k).to_string());
                    put("x-moniof-slowest-latency-ms", (**v).to_string());
                }
            }

            if cfg.log_warnings {
                let mut alerted = false;
                if total > cfg.max_total {
                    alerted = true;
                    tracing::warn!(target="moniof", total, max_total = cfg.max_total, elapsed_ms, db_total_ms, "High DB query count (possible N+1)");
                }
                if let Some((k, v)) = worst_count {
                    if *v > cfg.max_same_key {
                        alerted = true;
                        tracing::warn!(target="moniof", key=%k, count=*v, max_same_key = cfg.max_same_key, "Repeated same DB key (N+1 likely)");
                    }
                }
                if let Some(th) = cfg.warn_total_db_latency_ms {
                    if db_total_ms >= th {
                        alerted = true;
                        tracing::warn!(target="moniof", db_total_ms, threshold=th, "High cumulative DB latency in request");
                    }
                }
                if let Some(low) = cfg.warn_low_total_db_latency_ms {
                    if total > 0 && db_total_ms <= low {
                        alerted = true;
                        tracing::warn!(target="moniof", db_total_ms, threshold=low, total_queries=total, "Unexpectedly LOW cumulative DB latency (check instrumentation or cache?)");
                    }
                }

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
                        tokio::spawn(slack::notify(hook, lines.join("\n")));
                    }
                }
            }

            Ok(res)
        })
    }
}
