// Lightweight usage stats: per-day recording count + character count.
//
// Persisted as JSON next to the exe (`<exe_dir>/stats.json`). Updated
// synchronously on every successful transcription. Read by the Settings
// window's Stats tab.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DayStats {
    pub recordings: u64,
    pub chars: u64,
    #[serde(default, deserialize_with = "deser_finite_f64")]
    pub seconds: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatsFile {
    pub total_recordings: u64,
    pub total_chars: u64,
    #[serde(default, deserialize_with = "deser_finite_f64")]
    pub total_seconds: f64,
    /// `YYYY-MM-DD` → counts. BTreeMap so JSON dumps sorted.
    pub by_day: BTreeMap<String, DayStats>,
}

/// Treats JSON `null` (which serde_json writes when a f64 is NaN/±Inf —
/// see the AudioConfig::default sample_rate=0 bug) as 0.0 so we don't
/// throw away the entire stats.json on the first read after the fix.
fn deser_finite_f64<'de, D>(d: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<f64> = serde::Deserialize::deserialize(d).unwrap_or(Some(0.0));
    Ok(match v {
        Some(x) if x.is_finite() => x,
        _ => 0.0,
    })
}

pub struct Stats {
    path: PathBuf,
    inner: Mutex<StatsFile>,
}

impl Stats {
    pub fn open() -> Result<Self> {
        let path = stats_path()?;
        let inner = if path.exists() {
            let s = std::fs::read_to_string(&path)
                .with_context(|| format!("read {:?}", path))?;
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            StatsFile::default()
        };
        Ok(Self {
            path,
            inner: Mutex::new(inner),
        })
    }

    pub fn snapshot(&self) -> StatsFile {
        self.inner.lock().unwrap().clone()
    }

    /// Record one transcription. `chars` is the rendered transcript length;
    /// `audio_seconds` is the recording duration the user spent talking.
    pub fn record(&self, chars: u64, audio_seconds: f64) {
        let today = today_utc_date();
        {
            let mut s = self.inner.lock().unwrap();
            s.total_recordings += 1;
            s.total_chars += chars;
            s.total_seconds += audio_seconds;
            let entry = s.by_day.entry(today).or_default();
            entry.recordings += 1;
            entry.chars += chars;
            entry.seconds += audio_seconds;
            // Best-effort persistence; ignore IO errors but log.
            if let Err(e) = self.write_locked(&s) {
                tracing::warn!("stats write failed: {}", e);
            }
        }
    }

    fn write_locked(&self, s: &StatsFile) -> Result<()> {
        let json = serde_json::to_string_pretty(s).context("serialize stats")?;
        // Atomic-ish: write to .tmp then rename.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, json).with_context(|| format!("write {:?}", tmp))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename {:?} -> {:?}", tmp, self.path))?;
        Ok(())
    }
}

fn stats_path() -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()
        .context("current_exe failed")?
        .parent()
        .context("no parent")?
        .to_path_buf();
    Ok(exe_dir.join("stats.json"))
}

/// `YYYY-MM-DD` in UTC. Avoids a chrono dependency by computing manually.
/// Good enough for daily heat-map bucketing — switch to local TZ later if it
/// matters.
fn today_utc_date() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Convert "days since 1970-01-01 (UTC)" to (year, month, day).
/// Uses Howard Hinnant's civil-from-days algorithm.
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
