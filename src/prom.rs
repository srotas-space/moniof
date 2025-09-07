use actix_web::{HttpResponse};
use once_cell::sync::OnceCell;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntGauge, IntCounterVec, Opts, Registry, TextEncoder,
};

static REGISTRY: OnceCell<Registry> = OnceCell::new();

static HTTP_REQ_COUNTER: OnceCell<IntCounterVec> = OnceCell::new();
static HTTP_INFLIGHT: OnceCell<IntGauge> = OnceCell::new();
static HTTP_REQ_HISTO: OnceCell<HistogramVec> = OnceCell::new();

static DB_TOTAL_HISTO: OnceCell<HistogramVec> = OnceCell::new();
static MONGO_CMD_HISTO: OnceCell<HistogramVec> = OnceCell::new();

fn default_buckets_seconds() -> Vec<f64> {
    // Prometheus-default-ish buckets for latency (seconds)
    vec![0.005,0.01,0.025,0.05,0.1,0.25,0.5,1.0,2.5,5.0,10.0]
}

pub fn init_prometheus() {
    let registry = REGISTRY.get_or_init(Registry::new);

    let http_counter = IntCounterVec::new(
        Opts::new("moniof_http_requests_total", "HTTP requests total"),
        &["method", "status"],
    ).unwrap();

    let http_inflight = IntGauge::new("moniof_http_inflight_requests", "Inflight HTTP requests").unwrap();

    let http_histo = HistogramVec::new(
        HistogramOpts::new("moniof_http_request_duration_seconds", "HTTP request duration (s)")
            .buckets(default_buckets_seconds()),
        &["method"],
    ).unwrap();

    let db_total = HistogramVec::new(
        HistogramOpts::new("moniof_db_total_latency_seconds", "Cumulative DB latency per request (s)")
            .buckets(default_buckets_seconds()),
        &["kind"], // e.g., "mongo" (aggregate), room for "sql" later if you wish to split
    ).unwrap();

    let mongo_cmd = HistogramVec::new(
        HistogramOpts::new("moniof_mongo_command_duration_seconds", "Single Mongo command latency (s)")
            .buckets(default_buckets_seconds()),
        &["collection","op"],
    ).unwrap();

    registry.register(Box::new(http_counter.clone())).ok();
    registry.register(Box::new(http_inflight.clone())).ok();
    registry.register(Box::new(http_histo.clone())).ok();
    registry.register(Box::new(db_total.clone())).ok();
    registry.register(Box::new(mongo_cmd.clone())).ok();

    HTTP_REQ_COUNTER.set(http_counter).ok();
    HTTP_INFLIGHT.set(http_inflight).ok();
    HTTP_REQ_HISTO.set(http_histo).ok();
    DB_TOTAL_HISTO.set(db_total).ok();
    MONGO_CMD_HISTO.set(mongo_cmd).ok();
}

// Called by middleware
pub fn inc_inflight() {
    if let Some(g) = HTTP_INFLIGHT.get() { g.inc(); }
}
pub fn dec_inflight() {
    if let Some(g) = HTTP_INFLIGHT.get() { g.dec(); }
}
pub fn observe_request(method: &str, status: u16, dur_seconds: f64, db_total_seconds: f64) {
    if let Some(c) = HTTP_REQ_COUNTER.get() {
        c.with_label_values(&[method, &status.to_string()]).inc();
    }
    if let Some(h) = HTTP_REQ_HISTO.get() {
        h.with_label_values(&[method]).observe(dur_seconds);
    }
    if let Some(h) = DB_TOTAL_HISTO.get() {
        h.with_label_values(&["mongo"]).observe(db_total_seconds);
    }
}

// Called by mongo_events
pub fn observe_mongo_cmd(collection: &str, op: &str, dur_seconds: f64) {
    if let Some(h) = MONGO_CMD_HISTO.get() {
        h.with_label_values(&[collection, op]).observe(dur_seconds);
    }
}

pub async fn metrics_handler() -> HttpResponse {
    let Some(registry) = REGISTRY.get() else {
        init_prometheus();
        // try again
        let reg = REGISTRY.get().unwrap();
        return encode(reg);
    };
    encode(registry)
}

fn encode(registry: &Registry) -> HttpResponse {
    let encoder = TextEncoder::new();
    let mf = registry.gather();
    let mut buf = Vec::new();
    if let Err(e) = encoder.encode(&mf, &mut buf) {
        return HttpResponse::InternalServerError().body(format!("encode error: {e}"));
    }
    HttpResponse::Ok()
        .content_type(encoder.format_type())
        .body(buf)
}
