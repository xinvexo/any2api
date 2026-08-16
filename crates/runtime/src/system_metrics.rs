use std::{
    sync::Mutex,
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use sysinfo::{
    MINIMUM_CPU_UPDATE_INTERVAL, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate,
    System,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemMetricsSnapshot {
    pub sampled_at_ms: u64,
    pub process_resident_memory_bytes: u64,
    pub process_cpu_usage_percent: f32,
    pub system_used_memory_bytes: u64,
    pub system_total_memory_bytes: u64,
    pub system_cpu_usage_percent: f32,
}

#[derive(Debug, Error)]
pub enum SystemMetricsError {
    #[error("system metrics sampler lock is poisoned")]
    SamplerPoisoned,
    #[error("current process metrics are unavailable")]
    ProcessUnavailable,
    #[error("system memory metrics are unavailable")]
    MemoryUnavailable,
    #[error("system CPU metrics are unavailable")]
    CpuUnavailable,
    #[error("system metrics task failed")]
    Task(#[source] tokio::task::JoinError),
}

#[derive(Debug)]
pub(crate) struct SystemMetricsSampler {
    pid: Pid,
    state: Mutex<SystemMetricsState>,
}

#[derive(Debug)]
struct SystemMetricsState {
    system: System,
    last_cpu_refresh: Option<Instant>,
    last_snapshot: Option<SystemMetricsSnapshot>,
}

impl SystemMetricsSampler {
    pub(crate) fn new() -> Self {
        Self {
            pid: Pid::from_u32(std::process::id()),
            state: Mutex::new(SystemMetricsState {
                system: System::new(),
                last_cpu_refresh: None,
                last_snapshot: None,
            }),
        }
    }

    pub(crate) fn sample(&self) -> Result<SystemMetricsSnapshot, SystemMetricsError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SystemMetricsError::SamplerPoisoned)?;
        if let (Some(last_refresh), Some(snapshot)) = (state.last_cpu_refresh, state.last_snapshot)
            && last_refresh.elapsed() < MINIMUM_CPU_UPDATE_INTERVAL
        {
            return Ok(snapshot);
        }

        state
            .system
            .refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        refresh_process(&mut state.system, self.pid);
        state.system.refresh_cpu_usage();
        if state.last_cpu_refresh.is_none() {
            thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
            refresh_process(&mut state.system, self.pid);
            state.system.refresh_cpu_usage();
        }

        let process = state
            .system
            .process(self.pid)
            .ok_or(SystemMetricsError::ProcessUnavailable)?;
        let total_memory = state.system.total_memory();
        if total_memory == 0 {
            return Err(SystemMetricsError::MemoryUnavailable);
        }
        if state.system.cpus().is_empty() {
            return Err(SystemMetricsError::CpuUnavailable);
        }
        let process_cpu = normalize_cpu_percent(process.cpu_usage(), state.system.cpus().len())?;
        let system_cpu = normalize_cpu_percent(state.system.global_cpu_usage(), 1)?;

        let snapshot = SystemMetricsSnapshot {
            sampled_at_ms: unix_millis(),
            process_resident_memory_bytes: process.memory(),
            process_cpu_usage_percent: process_cpu,
            system_used_memory_bytes: state.system.used_memory(),
            system_total_memory_bytes: total_memory,
            system_cpu_usage_percent: system_cpu,
        };
        state.last_cpu_refresh = Some(Instant::now());
        state.last_snapshot = Some(snapshot);
        Ok(snapshot)
    }
}

fn refresh_process(system: &mut System, pid: Pid) {
    let pids = [pid];
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
            .without_tasks(),
    );
}

fn normalize_cpu_percent(value: f32, divisor: usize) -> Result<f32, SystemMetricsError> {
    if !value.is_finite() || value < 0.0 {
        return Err(SystemMetricsError::CpuUnavailable);
    }
    Ok((value / divisor.max(1) as f32).clamp(0.0, 100.0))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{SystemMetricsError, normalize_cpu_percent};

    #[test]
    fn normalizes_process_cpu_to_host_capacity() {
        assert_eq!(normalize_cpu_percent(200.0, 4).expect("valid CPU"), 50.0);
        assert_eq!(normalize_cpu_percent(800.0, 4).expect("valid CPU"), 100.0);
    }

    #[test]
    fn rejects_invalid_cpu_values() {
        assert!(matches!(
            normalize_cpu_percent(f32::NAN, 4),
            Err(SystemMetricsError::CpuUnavailable)
        ));
        assert!(matches!(
            normalize_cpu_percent(-1.0, 4),
            Err(SystemMetricsError::CpuUnavailable)
        ));
    }
}
