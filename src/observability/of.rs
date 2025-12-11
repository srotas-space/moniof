use crate::config::MoniOFConfig;
use crate::core::stats::QueryStats;

#[derive(Debug, Clone)]
pub struct OfSuspect {
    pub key: String,
    pub count: usize,
    pub total_latency_ms: u128,
}

pub fn find_suspects(stats: &QueryStats, cfg: &MoniOFConfig) -> Vec<OfSuspect> {
    if !cfg.of_mode {
        return Vec::new();
    }

    let mut suspects = Vec::new();

    for (k, count) in &stats.per_key {
        if *count < cfg.n_plus_one_min_count {
            continue;
        }

        let total_ms = stats.per_key_latency_ms.get(k).copied().unwrap_or(0);

        if let Some(min_ms) = cfg.n_plus_one_min_total_ms {
            if total_ms < min_ms {
                continue;
            }
        }

        suspects.push(OfSuspect {
            key: k.clone(),
            count: *count,
            total_latency_ms: total_ms,
        });
    }

    suspects.sort_by(|a, b| {
        b.count.cmp(&a.count).then_with(|| b.total_latency_ms.cmp(&a.total_latency_ms))
    });

    if suspects.len() > 3 {
        suspects.truncate(3);
    }

    suspects
}
