// optimized_lock.rs - 优化的锁管理器，减少锁竞争

use rustfs_lock::{LockGuard, NamespaceLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::debug;

/// 锁统计信息
#[derive(Debug, Clone, Default)]
pub struct LockStats {
    /// 总请求数
    pub total_requests: u64,
    /// 成功获取数
    pub successful_acquires: u64,
    /// 等待超时数
    pub timeouts: u64,
    /// 平均等待时间
    pub avg_wait_time: Duration,
    /// 最大等待时间
    pub max_wait_time: Duration,
}

/// 锁优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LockPriority {
    /// 低优先级（如扫描操作）
    Low = 0,
    /// 正常优先级（如读操作）
    Normal = 1,
    /// 高优先级（如写操作）
    High = 2,
    /// 紧急优先级（如关键路径写入）
    Critical = 3,
}

/// 优化的锁请求
pub struct LockRequest {
    pub key: String,
    pub priority: LockPriority,
    pub timeout: Duration,
    pub is_write: bool,
}

/// 优化的锁管理器
pub struct OptimizedLockManager {
    /// 底层命名空间锁
    namespace_lock: Arc<NamespaceLock>,
    /// 锁统计信息
    stats: Arc<RwLock<HashMap<String, LockStats>>>,
    /// 并发限制
    semaphore: Arc<Semaphore>,
    /// 热点检测缓存
    hot_keys: Arc<RwLock<HashMap<String, HotKeyInfo>>>,
    /// 是否启用优化
    optimization_enabled: bool,
}

/// 热点键信息
#[derive(Debug, Clone)]
struct HotKeyInfo {
    /// 访问计数
    access_count: u64,
    /// 上次访问时间
    last_access: Instant,
    /// 是否是热点
    is_hot: bool,
}

impl OptimizedLockManager {
    pub fn new(namespace_lock: Arc<NamespaceLock>, max_concurrent: usize) -> Self {
        Self {
            namespace_lock,
            stats: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            hot_keys: Arc::new(RwLock::new(HashMap::new())),
            optimization_enabled: true,
        }
    }

    /// 获取优化的锁
    pub async fn acquire_lock(&self, request: LockRequest, owner: &str) -> Result<Option<LockGuard>, String> {
        // 记录请求开始时间
        let start = Instant::now();

        // 更新热点检测
        self.update_hot_key(&request.key).await;

        // 检查是否是热点键
        let is_hot = self.is_hot_key(&request.key).await;

        // 如果是低优先级且键是热点，快速失败
        if request.priority == LockPriority::Low && is_hot {
            debug!("Skipping lock acquisition for hot key {} with low priority", request.key);
            self.record_timeout(&request.key).await;
            return Ok(None);
        }

        // 获取信号量许可
        let _permit = match request.priority {
            LockPriority::Critical => {
                // 紧急优先级不等待
                self.semaphore
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| "Too many concurrent lock requests")?
            }
            _ => {
                // 其他优先级等待
                match tokio::time::timeout(request.timeout / 2, self.semaphore.clone().acquire_owned()).await {
                    Ok(Ok(permit)) => permit,
                    _ => {
                        self.record_timeout(&request.key).await;
                        return Ok(None);
                    }
                }
            }
        };

        // 调整超时时间
        let adjusted_timeout = self.adjust_timeout(request.timeout, request.priority, is_hot);

        // 获取实际的锁
        let result = if request.is_write {
            self.namespace_lock
                .lock_guard(&request.key, owner, adjusted_timeout, Duration::from_secs(10))
                .await
        } else {
            self.namespace_lock
                .rlock_guard(&request.key, owner, adjusted_timeout, Duration::from_secs(10))
                .await
        };

        // 记录统计信息
        let wait_time = start.elapsed();
        self.record_stats(&request.key, wait_time, result.is_ok()).await;

        result.map_err(|e| e.to_string())
    }

    /// 更新热点键信息
    async fn update_hot_key(&self, key: &str) {
        let mut hot_keys = self.hot_keys.write().await;
        let info = hot_keys.entry(key.to_string()).or_insert(HotKeyInfo {
            access_count: 0,
            last_access: Instant::now(),
            is_hot: false,
        });

        info.access_count += 1;
        info.last_access = Instant::now();

        // 简单的热点检测：1秒内访问超过10次
        if info.access_count > 10 {
            info.is_hot = true;
        }

        // 定期重置计数
        if info.last_access.elapsed() > Duration::from_secs(1) {
            info.access_count = 1;
            info.is_hot = false;
        }
    }

    /// 检查是否是热点键
    async fn is_hot_key(&self, key: &str) -> bool {
        let hot_keys = self.hot_keys.read().await;
        hot_keys.get(key).map_or(false, |info| info.is_hot)
    }

    /// 调整超时时间
    fn adjust_timeout(&self, base_timeout: Duration, priority: LockPriority, is_hot: bool) -> Duration {
        if !self.optimization_enabled {
            return base_timeout;
        }

        let multiplier = match priority {
            LockPriority::Critical => 2.0, // 紧急优先级等待更长
            LockPriority::High => 1.5,
            LockPriority::Normal => 1.0,
            LockPriority::Low => 0.5, // 低优先级快速失败
        };

        let adjusted = if is_hot {
            // 热点键减少等待时间
            Duration::from_secs_f64(base_timeout.as_secs_f64() * multiplier * 0.5)
        } else {
            Duration::from_secs_f64(base_timeout.as_secs_f64() * multiplier)
        };

        // 确保最小和最大值
        adjusted.clamp(Duration::from_millis(100), Duration::from_secs(30))
    }

    /// 记录统计信息
    async fn record_stats(&self, key: &str, wait_time: Duration, success: bool) {
        let mut stats_map = self.stats.write().await;
        let stats = stats_map.entry(key.to_string()).or_insert(LockStats::default());

        stats.total_requests += 1;
        if success {
            stats.successful_acquires += 1;
        } else {
            stats.timeouts += 1;
        }

        // 更新平均等待时间
        let n = stats.successful_acquires as f64;
        if n > 0.0 {
            let current_avg = stats.avg_wait_time.as_secs_f64();
            let new_avg = (current_avg * (n - 1.0) + wait_time.as_secs_f64()) / n;
            stats.avg_wait_time = Duration::from_secs_f64(new_avg);
        }

        // 更新最大等待时间
        if wait_time > stats.max_wait_time {
            stats.max_wait_time = wait_time;
        }
    }

    /// 记录超时
    async fn record_timeout(&self, key: &str) {
        let mut stats_map = self.stats.write().await;
        let stats = stats_map.entry(key.to_string()).or_insert(LockStats::default());
        stats.timeouts += 1;
    }

    /// 获取锁统计信息
    pub async fn get_stats(&self, key: &str) -> Option<LockStats> {
        let stats_map = self.stats.read().await;
        stats_map.get(key).cloned()
    }

    /// 清理过期的热点信息
    pub async fn cleanup_hot_keys(&self) {
        let mut hot_keys = self.hot_keys.write().await;
        let now = Instant::now();
        hot_keys.retain(|_, info| now.duration_since(info.last_access) < Duration::from_secs(60));
    }

    /// 获取热点键列表
    pub async fn get_hot_keys(&self) -> Vec<String> {
        let hot_keys = self.hot_keys.read().await;
        hot_keys
            .iter()
            .filter(|(_, info)| info.is_hot)
            .map(|(key, _)| key.clone())
            .collect()
    }
}
