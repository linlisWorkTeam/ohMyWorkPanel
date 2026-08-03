//! Periodic process RSS / CPU sampling into `logs` (source=`perf`).

use crate::logger::{self, LogLevel};
use crate::db::open_db;
use std::path::PathBuf;
use std::time::Duration;

/// WorkPanel process targets (excludes child Agent CLIs).
pub const RSS_WARN_MIB: f64 = 120.0;
pub const RSS_CRIT_MIB: f64 = 200.0;
pub const CPU_WARN_PCT: f64 = 25.0;
pub const CPU_CRIT_PCT: f64 = 50.0;

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub rss_mib: f64,
    pub cpu_pct: f64,
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
    // fields after comm: 3=state ... 14=utime(idx11), 15=stime(idx12) — 0-based after ') '
    let utime: u64 = parts.get(11)?.parse().ok()?;
    let stime: u64 = parts.get(12)?.parse().ok()?;
    Some(utime.saturating_add(stime))
}

pub fn ticks_per_second() -> f64 {
    // Linux default USER_HZ; avoid libc dep on 2GB hosts.
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

pub fn start_perf_loop(db_path: PathBuf) {
    tokio::spawn(async move {
        let mut prev_ticks = read_cpu_ticks();
        let mut prev_at = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
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
            };
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
                cpu_pct: 1.0
            })
            .0,
            "ok"
        );
        assert_eq!(
            classify(Sample {
                rss_mib: 130.0,
                cpu_pct: 1.0
            })
            .0,
            "warn"
        );
        assert_eq!(
            classify(Sample {
                rss_mib: 50.0,
                cpu_pct: 55.0
            })
            .0,
            "crit"
        );
    }
}
