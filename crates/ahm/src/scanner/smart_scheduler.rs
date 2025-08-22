// smart_scheduler.rs - 智能扫描调度器

use super::scanner_config::{ScannerMode, ScannerPerfConfig};
use rustfs_ecstore::perf_monitor::{LoadStatus, PerfMonitor};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 智能调度器状态
#[derive(Debug, Clone)]
pub struct SchedulerState {
    /// 当前模式
    pub current_mode: ScannerMode,
    /// 上次扫描时间
    pub last_scan_time: Option<Instant>,
    /// 暂停到期时间
    pub pause_until: Option<Instant>,
    /// 连续高负载计数
    pub high_load_count: u32,
    /// 连续低负载计数
    pub low_load_count: u32,
}

/// 智能扫描调度器
pub struct SmartScheduler {
    config: Arc<RwLock<ScannerPerfConfig>>,
    state: Arc<RwLock<SchedulerState>>,
    perf_monitor: Arc<PerfMonitor>,
}

impl SmartScheduler {
    pub fn new(config: ScannerPerfConfig, perf_monitor: Arc<PerfMonitor>) -> Self {
        Self {
            config: Arc::new(RwLock::new(config.clone())),
            state: Arc::new(RwLock::new(SchedulerState {
                current_mode: config.mode,
                last_scan_time: None,
                pause_until: None,
                high_load_count: 0,
                low_load_count: 0,
            })),
            perf_monitor,
        }
    }

    /// 检查是否应该开始扫描
    pub async fn should_scan(&self) -> bool {
        let config = self.config.read().await;
        let mut state = self.state.write().await;

        // 检查模式
        if config.mode == ScannerMode::Disabled {
            debug!("Scanner is disabled");
            return false;
        }

        // 检查暂停状态
        if let Some(pause_until) = state.pause_until {
            if Instant::now() < pause_until {
                debug!("Scanner is paused until {:?}", pause_until);
                return false;
            }
            state.pause_until = None;
        }

        // 检查最小扫描间隔
        if let Some(last_scan) = state.last_scan_time {
            if last_scan.elapsed() < config.min_scan_interval {
                debug!("Minimum scan interval not reached");
                return false;
            }
        }

        // 检查系统负载
        let load_status = self.perf_monitor.get_load_status().await;
        let current_iops = self.perf_monitor.get_current_iops().await;

        match config.mode {
            ScannerMode::Disabled => false,
            ScannerMode::LowLoadOnly => {
                // 只在低负载时扫描
                if load_status == LoadStatus::Idle || load_status == LoadStatus::Low {
                    state.low_load_count += 1;
                    state.high_load_count = 0;

                    // 连续低负载3次后才开始扫描
                    if state.low_load_count >= 3 {
                        info!("Low load detected, starting scan");
                        true
                    } else {
                        false
                    }
                } else {
                    state.high_load_count += 1;
                    state.low_load_count = 0;

                    // 高负载时暂停
                    if state.high_load_count >= 2 {
                        let pause_duration = config.pause_duration;
                        state.pause_until = Some(Instant::now() + pause_duration);
                        warn!("High load detected (IOPS: {}), pausing scanner for {:?}", current_iops, pause_duration);
                    }
                    false
                }
            }
            ScannerMode::Normal => {
                // 正常模式，但仍然考虑负载
                if load_status == LoadStatus::Overload {
                    state.high_load_count += 1;
                    if state.high_load_count >= 3 {
                        state.pause_until = Some(Instant::now() + Duration::from_secs(10));
                        warn!("System overloaded, pausing scanner");
                    }
                    false
                } else {
                    state.high_load_count = 0;
                    true
                }
            }
            ScannerMode::Aggressive => {
                // 激进模式，总是扫描
                true
            }
        }
    }

    /// 记录扫描开始
    pub async fn record_scan_start(&self) {
        let mut state = self.state.write().await;
        state.last_scan_time = Some(Instant::now());
    }

    /// 根据系统负载动态调整配置
    pub async fn adaptive_adjust(&self) {
        let config = self.config.read().await;
        if !config.smart_scheduling {
            return;
        }

        let load_status = self.perf_monitor.get_load_status().await;
        let current_iops = self.perf_monitor.get_current_iops().await;

        drop(config);
        let mut config = self.config.write().await;

        // 根据负载动态调整参数
        match load_status {
            LoadStatus::Idle => {
                // 空闲时可以更激进
                config.batch_size = (config.batch_size * 2).min(200);
                config.batch_interval = Duration::from_millis(50);
                debug!("System idle, increasing scan batch size to {}", config.batch_size);
            }
            LoadStatus::Low => {
                // 低负载时正常扫描
                config.batch_size = 100;
                config.batch_interval = Duration::from_millis(100);
            }
            LoadStatus::Medium => {
                // 中等负载时减少扫描强度
                config.batch_size = (config.batch_size / 2).max(20);
                config.batch_interval = Duration::from_millis(200);
                debug!("Medium load, reducing scan batch size to {}", config.batch_size);
            }
            LoadStatus::High | LoadStatus::Overload => {
                // 高负载时最小化扫描
                config.batch_size = 10;
                config.batch_interval = Duration::from_millis(500);
                warn!("High load (IOPS: {}), minimizing scan intensity", current_iops);
            }
        }
    }

    /// 获取当前扫描批次大小
    pub async fn get_batch_size(&self) -> usize {
        self.config.read().await.batch_size
    }

    /// 获取批次间隔
    pub async fn get_batch_interval(&self) -> Duration {
        self.config.read().await.batch_interval
    }

    /// 检查是否应该跳过最近修改的对象
    pub async fn should_skip_recent(&self, modified_time: Instant) -> bool {
        let config = self.config.read().await;
        modified_time.elapsed() < config.skip_recent_threshold
    }
}
