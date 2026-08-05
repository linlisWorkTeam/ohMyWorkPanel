//! Periodic process RSS / CPU sampling into `logs` (source=`perf`).

use crate::db::open_db;
use crate::logger::{self, LogLevel};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// WorkPanel process targets (excludes child Agent CLIs).
pub const RSS_WARN_MIB: f64 = 120.0;
pub const RSS_CRIT_MIB: f64 = 200.0;
pub const CPU_WARN_PCT: f64 = 25.0;
pub const CPU_CRIT_PCT: f64 = 50.0;
pub const PERF_SAMPLE_SECS: u64 = 20;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub rss_mib: f64,
    pub cpu_pct: f64,
    pub ts: i64,
}

static LATEST: OnceLock<Mutex<Option<Sample>>> = OnceLock::new();

fn latest_slot() -> &'static Mutex<Option<Sample>> {
    LATEST.get_or_init(|| Mutex::new(None))
}

pub fn latest_sample() -> Option<Sample> {
    latest_slot().lock().ok().and_then(|g| *g)
}

pub fn store_latest(sample: Sample) {
    if let Ok(mut g) = latest_slot().lock() {
        *g = Some(sample);
    }
}

pub fn read_rss_mib() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: f64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024.0);
        }
    }
    None
}

/// Returns (utime+stime ticks) from /proc/self/stat fields 14+15.
pub fn read_cpu_ticks() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let start = stat.rfind(')')?;
    let after = stat.get(start + 2..)?;
    let parts: Vec<&str> = after.split_whitespace().collect();
    let utime: u64 = parts.get(11)?.parse().ok()?;
    let stime: u64 = parts.get(12)?.parse().ok()?;
    Some(utime.saturating_add(stime))
}

pub fn ticks_per_second() -> f64 {
    100.0
}

pub fn classify(sample: Sample) -> (&'static str, LogLevel) {
    if sample.rss_mib > RSS_CRIT_MIB || sample.cpu_pct > CPU_CRIT_PCT {
        ("crit", LogLevel::Warn)
    } else if sample.rss_mib > RSS_WARN_MIB || sample.cpu_pct > CPU_WARN_PCT {
        ("warn", LogLevel::Warn)
    } else {
        ("ok", LogLevel::Info)
    }
}

/// Prefer in-memory latest; fall back to last perf log row.
pub fn latest_or_from_db(db_path: &std::path::Path) -> Option<Sample> {
    if let Some(s) = latest_sample() {
        return Some(s);
    }
    let conn = open_db(db_path).ok()?;
    let details: String = conn
        .query_row(
            "SELECT details FROM logs WHERE source='perf' ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok()?;
    let v: serde_json::Value = serde_json::from_str(&details).ok()?;
    Some(Sample {
        rss_mib: v.get("rssMb")?.as_f64()?,
        cpu_pct: v.get("cpuPct")?.as_f64()?,
        ts: crate::db::now(),
    })
}

pub fn start_perf_loop(db_path: PathBuf) {
    tokio::spawn(async move {
        let mut prev_ticks = read_cpu_ticks();
        let mut prev_at = std::time::Instant::now();
        // First sample sooner so /api/metrics/latest is useful after boot.
        tokio::time::sleep(Duration::from_secs(2)).await;
        loop {
            let rss = read_rss_mib().unwrap_or(0.0);
            let now_ticks = read_cpu_ticks();
            let elapsed = prev_at.elapsed().as_secs_f64().max(0.001);
            let cpu_pct = match (prev_ticks, now_ticks) {
                (Some(a), Some(b)) if b >= a => {
                    let delta = (b - a) as f64;
                    (delta / ticks_per_second() / elapsed) * 100.0
                }
                _ => 0.0,
            };
            prev_ticks = now_ticks;
            prev_at = std::time::Instant::now();
            let sample = Sample {
                rss_mib: rss,
                cpu_pct,
                ts: crate::db::now(),
            };
            store_latest(sample);
            let (tag, level) = classify(sample);
            let msg = format!(
                "perf {tag}: rss_mb={:.1} cpu_pct={:.1} (targets rss≤80 warn>{} crit>{} cpu_warn>{} crit>{})",
                sample.rss_mib,
                sample.cpu_pct,
                RSS_WARN_MIB,
                RSS_CRIT_MIB,
                CPU_WARN_PCT,
                CPU_CRIT_PCT
            );
            let details = format!(
                "{{\"rssMb\":{:.2},\"cpuPct\":{:.2},\"tag\":\"{}\"}}",
                sample.rss_mib, sample.cpu_pct, tag
            );
            if let Ok(conn) = open_db(&db_path) {
                let _ = logger::log(&conn, level, "perf", &msg, Some(&details));
            }
            tokio::time::sleep(Duration::from_secs(PERF_SAMPLE_SECS)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_thresholds() {
        assert_eq!(
            classify(Sample {
                rss_mib: 50.0,
                cpu_pct: 1.0,
                ts: 0
            })
            .0,
            "ok"
        );
        assert_eq!(
            classify(Sample {
                rss_mib: 130.0,
                cpu_pct: 1.0,
                ts: 0
            })
            .0,
            "warn"
        );
        assert_eq!(
            classify(Sample {
                rss_mib: 50.0,
                cpu_pct: 55.0,
                ts: 0
            })
            .0,
            "crit"
        );
    }

    #[test]
    fn stores_latest_sample() {
        store_latest(Sample {
            rss_mib: 12.0,
            cpu_pct: 3.0,
            ts: 42,
        });
        let s = latest_sample().unwrap();
        assert_eq!(s.ts, 42);
        assert!((s.rss_mib - 12.0).abs() < 0.01);
    }

    #[test]
    fn perf_sample_interval_is_20s() {
        assert_eq!(PERF_SAMPLE_SECS, 20);
    }
}
