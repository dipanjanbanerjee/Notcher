//! System vitals: CPU, memory, disk, network and battery.
//!
//! Sampled once a second on a background thread with a minute of history kept in
//! memory. Updates are only sent to the UI while the island panel is open.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};
use tauri::{AppHandle, Emitter, Runtime};

pub const VITALS_EVENT: &str = "vitals://update";
const HISTORY: usize = 60;
const SAMPLE_EVERY: Duration = Duration::from_secs(1);
const DISK_EVERY: Duration = Duration::from_secs(30);

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LATEST: Mutex<Option<Vitals>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: f32,
    pub charging: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Vitals {
    pub cpu: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub net_rx: u64,
    pub net_tx: u64,
    pub battery: Option<Battery>,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
}

/// Fixed-length history of percentages.
#[derive(Default)]
struct Ring(VecDeque<f32>);

impl Ring {
    fn push(&mut self, v: f32) {
        if self.0.len() == HISTORY {
            self.0.pop_front();
        }
        self.0.push_back(v);
    }
    fn to_vec(&self) -> Vec<f32> {
        self.0.iter().copied().collect()
    }
}

/// Used and total bytes of the disk holding the user's files (the largest one if unsure).
fn main_disk(disks: &Disks) -> (u64, u64) {
    let home = dirs_home();
    let pick = disks
        .iter()
        .filter(|d| {
            home.as_ref()
                .is_some_and(|h| h.starts_with(d.mount_point()))
        })
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .or_else(|| disks.iter().max_by_key(|d| d.total_space()));
    pick.map(|d| {
        (
            d.total_space().saturating_sub(d.available_space()),
            d.total_space(),
        )
    })
    .unwrap_or((0, 0))
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(Into::into)
}

pub fn start<R: Runtime>(app: AppHandle<R>) {
    let spawned = std::thread::Builder::new()
        .name("notcher-vitals".into())
        .spawn(move || {
            let (Ok(mut sys), Ok(mut networks), Ok(mut disks)) = (
                System::new(),
                Networks::new_with_refreshed_list(),
                Disks::new_with_refreshed_list(),
            ) else {
                log::warn!("system vitals unavailable on this system");
                return;
            };
            let mut disk = main_disk(&disks);
            let mut last_disk = Instant::now();
            let mut last_net = Instant::now();
            let (mut cpu_hist, mut mem_hist) = (Ring::default(), Ring::default());
            sys.refresh_cpu_usage();
            loop {
                std::thread::sleep(SAMPLE_EVERY);
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                networks.refresh(true);
                let secs = last_net.elapsed().as_secs_f64().max(0.001);
                last_net = Instant::now();
                let (rx, tx) = networks.iter().fold((0u64, 0u64), |(r, t), (_, n)| {
                    (r + n.received(), t + n.transmitted())
                });
                if last_disk.elapsed() >= DISK_EVERY {
                    disks.refresh(true);
                    disk = main_disk(&disks);
                    last_disk = Instant::now();
                }
                let cpu = sys.global_cpu_usage().clamp(0.0, 100.0);
                let (mem_used, mem_total) = (sys.used_memory(), sys.total_memory());
                let mem_pct = if mem_total > 0 {
                    mem_used as f32 / mem_total as f32 * 100.0
                } else {
                    0.0
                };
                cpu_hist.push(cpu);
                mem_hist.push(mem_pct);
                let vitals = Vitals {
                    cpu,
                    mem_used,
                    mem_total,
                    disk_used: disk.0,
                    disk_total: disk.1,
                    net_rx: (rx as f64 / secs) as u64,
                    net_tx: (tx as f64 / secs) as u64,
                    battery: battery(),
                    cpu_history: cpu_hist.to_vec(),
                    mem_history: mem_hist.to_vec(),
                };
                if ACTIVE.load(Ordering::Relaxed) {
                    let _ = app.emit(VITALS_EVENT, &vitals);
                }
                if let Ok(mut latest) = LATEST.lock() {
                    *latest = Some(vitals);
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("could not start vitals sampler: {err}");
    }
}

#[cfg(target_os = "linux")]
fn battery() -> Option<Battery> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    let mut total = (0f32, 0u32);
    let mut charging = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = std::fs::read_to_string(path.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        if let Some(cap) = std::fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|c| c.trim().parse::<f32>().ok())
        {
            total.0 += cap;
            total.1 += 1;
        }
        let status = std::fs::read_to_string(path.join("status")).unwrap_or_default();
        charging |= matches!(status.trim(), "Charging" | "Full");
    }
    (total.1 > 0).then(|| Battery {
        percent: total.0 / total.1 as f32,
        charging,
    })
}

#[cfg(windows)]
fn battery() -> Option<Battery> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    // 128 = no system battery, 255 = unknown.
    if status.BatteryFlag & 128 != 0 || status.BatteryLifePercent == 255 {
        return None;
    }
    Some(Battery {
        percent: status.BatteryLifePercent as f32,
        charging: status.ACLineStatus == 1,
    })
}

#[cfg(not(any(target_os = "linux", windows)))]
fn battery() -> Option<Battery> {
    None
}

#[tauri::command]
pub fn vitals_snapshot() -> Option<Vitals> {
    LATEST.lock().ok().and_then(|v| v.clone())
}

#[tauri::command]
pub fn set_vitals_active(active: bool) {
    ACTIVE.store(active, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_keeps_last_minute() {
        let mut r = Ring::default();
        for i in 0..(HISTORY + 10) {
            r.push(i as f32);
        }
        let v = r.to_vec();
        assert_eq!(v.len(), HISTORY);
        assert_eq!(v[0], 10.0);
        assert_eq!(*v.last().unwrap(), (HISTORY + 9) as f32);
    }
}
