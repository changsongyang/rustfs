// scanner_config.rs - Scanner 性能优化配置

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Scanner 运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScannerMode {
    /// 禁用扫描器
    Disabled,
    /// 仅在低负载时扫描
    LowLoadOnly,
    /// 正常扫描（默认）
    Normal,
    /// 激进扫描（用于数据恢复）
    Aggressive,
}

/// Scanner 性能配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerPerfConfig {
    /// 扫描器运行模式
    pub mode: ScannerMode,

    /// 写入负载阈值（IOPS），超过此值时暂停扫描
    pub write_load_threshold: u64,

    /// 扫描暂停时间（当检测到高写入负载时）
    pub pause_duration: Duration,

    /// 扫描批次大小（每批扫描的对象数）
    pub batch_size: usize,

    /// 批次间隔时间
    pub batch_interval: Duration,

    /// 是否启用智能调度（根据系统负载动态调整）
    pub smart_scheduling: bool,

    /// 最小扫描间隔（避免过于频繁的扫描）
    pub min_scan_interval: Duration,

    /// 跳过最近修改的对象（减少对热数据的干扰）
    pub skip_recent_threshold: Duration,

    /// 使用只读锁进行扫描（减少锁竞争）
    pub use_read_locks: bool,

    /// 扫描优先级（0-10，0最低）
    pub priority: u8,
}

impl Default for ScannerPerfConfig {
    fn default() -> Self {
        Self {
            mode: ScannerMode::LowLoadOnly,
            write_load_threshold: 1000, // 1000 IOPS
            pause_duration: Duration::from_secs(30),
            batch_size: 100,
            batch_interval: Duration::from_millis(100),
            smart_scheduling: true,
            min_scan_interval: Duration::from_secs(300),    // 5分钟
            skip_recent_threshold: Duration::from_secs(60), // 跳过1分钟内修改的
            use_read_locks: true,
            priority: 2,
        }
    }
}

impl ScannerPerfConfig {
    /// 为高写入负载场景优化的配置
    pub fn for_high_write_load() -> Self {
        Self {
            mode: ScannerMode::LowLoadOnly,
            write_load_threshold: 500,
            pause_duration: Duration::from_secs(60),
            batch_size: 50,
            batch_interval: Duration::from_millis(200),
            smart_scheduling: true,
            min_scan_interval: Duration::from_secs(600),
            skip_recent_threshold: Duration::from_secs(300),
            use_read_locks: true,
            priority: 1,
        }
    }

    /// 为低延迟场景优化的配置
    pub fn for_low_latency() -> Self {
        Self {
            mode: ScannerMode::Disabled,
            write_load_threshold: 100,
            pause_duration: Duration::from_secs(120),
            batch_size: 10,
            batch_interval: Duration::from_millis(500),
            smart_scheduling: true,
            min_scan_interval: Duration::from_secs(1800),
            skip_recent_threshold: Duration::from_secs(600),
            use_read_locks: true,
            priority: 0,
        }
    }
}
