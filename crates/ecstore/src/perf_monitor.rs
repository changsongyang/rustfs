// perf_monitor.rs - 写入性能监控和负载检测

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 性能指标
#[derive(Debug, Clone)]
pub struct PerfMetrics {
    /// 当前 IOPS
    pub current_iops: f64,
    /// 当前吞吐量 (MB/s)
    pub current_throughput: f64,
    /// 平均写入延迟
    pub avg_write_latency: Duration,
    /// P99 写入延迟
    pub p99_write_latency: Duration,
    /// 队列深度
    pub queue_depth: usize,
    /// CPU 使用率 (0-100)
    pub cpu_usage: f64,
    /// 内存使用率 (0-100)
    pub memory_usage: f64,
}

/// 负载状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    /// 空闲
    Idle,
    /// 低负载
    Low,
    /// 中等负载
    Medium,
    /// 高负载
    High,
    /// 过载
    Overload,
}

/// 性能监控器
pub struct PerfMonitor {
    /// 写入计数器
    write_count: Arc<AtomicU64>,
    /// 字节计数器
    bytes_written: Arc<AtomicU64>,
    /// 最近的指标
    recent_metrics: Arc<RwLock<PerfMetrics>>,
    /// 采样窗口
    sample_window: Duration,
    /// 上次采样时间
    last_sample: Arc<RwLock<Instant>>,
}

impl PerfMonitor {
    pub fn new() -> Self {
        Self {
            write_count: Arc::new(AtomicU64::new(0)),
            bytes_written: Arc::new(AtomicU64::new(0)),
            recent_metrics: Arc::new(RwLock::new(PerfMetrics {
                current_iops: 0.0,
                current_throughput: 0.0,
                avg_write_latency: Duration::ZERO,
                p99_write_latency: Duration::ZERO,
                queue_depth: 0,
                cpu_usage: 0.0,
                memory_usage: 0.0,
            })),
            sample_window: Duration::from_secs(1),
            last_sample: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// 记录写入操作
    pub fn record_write(&self, bytes: usize, _latency: Duration) {
        self.write_count.fetch_add(1, Ordering::Relaxed);
        self.bytes_written.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// 更新指标
    pub async fn update_metrics(&self) {
        let now = Instant::now();
        let mut last_sample = self.last_sample.write().await;
        let elapsed = now.duration_since(*last_sample);

        if elapsed < self.sample_window {
            return;
        }

        let writes = self.write_count.swap(0, Ordering::Relaxed);
        let bytes = self.bytes_written.swap(0, Ordering::Relaxed);

        let iops = writes as f64 / elapsed.as_secs_f64();
        let throughput = (bytes as f64 / 1024.0 / 1024.0) / elapsed.as_secs_f64();

        let mut metrics = self.recent_metrics.write().await;
        metrics.current_iops = iops;
        metrics.current_throughput = throughput;

        // 更新 CPU 和内存使用率（简化实现）
        metrics.cpu_usage = Self::get_cpu_usage();
        metrics.memory_usage = Self::get_memory_usage();

        *last_sample = now;
    }

    /// 获取当前负载状态
    pub async fn get_load_status(&self) -> LoadStatus {
        let metrics = self.recent_metrics.read().await;

        // 基于 IOPS 和 CPU 使用率判断负载
        if metrics.current_iops < 100.0 && metrics.cpu_usage < 20.0 {
            LoadStatus::Idle
        } else if metrics.current_iops < 500.0 && metrics.cpu_usage < 40.0 {
            LoadStatus::Low
        } else if metrics.current_iops < 1000.0 && metrics.cpu_usage < 60.0 {
            LoadStatus::Medium
        } else if metrics.current_iops < 5000.0 && metrics.cpu_usage < 80.0 {
            LoadStatus::High
        } else {
            LoadStatus::Overload
        }
    }

    /// 获取当前 IOPS
    pub async fn get_current_iops(&self) -> f64 {
        self.recent_metrics.read().await.current_iops
    }

    /// 是否应该暂停扫描
    pub async fn should_pause_scan(&self, threshold: u64) -> bool {
        let metrics = self.recent_metrics.read().await;
        metrics.current_iops > threshold as f64 || metrics.cpu_usage > 70.0
    }

    // 简化的 CPU 使用率获取
    fn get_cpu_usage() -> f64 {
        // 实际实现应该使用 sysinfo 或类似库
        // 这里返回模拟值
        30.0
    }

    // 简化的内存使用率获取
    fn get_memory_usage() -> f64 {
        // 实际实现应该使用 sysinfo 或类似库
        // 这里返回模拟值
        40.0
    }
}

impl Default for PerfMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局性能监控器实例
lazy_static::lazy_static! {
    pub static ref GLOBAL_PERF_MONITOR: PerfMonitor = PerfMonitor::new();
}

/// 启动性能监控后台任务
pub fn start_perf_monitoring() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            GLOBAL_PERF_MONITOR.update_metrics().await;
        }
    });
}
