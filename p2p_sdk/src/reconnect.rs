//! 重连工具函数
//!
//! 提供指数退避重连计算功能

use rand::Rng;

/// 重连配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReconnectConfig {
    /// 首次重连等待时间（秒）
    #[serde(default = "default_initial_interval")]
    pub initial_interval: u64,

    /// 最大重连间隔（秒）
    #[serde(default = "default_max_interval")]
    pub max_interval: u64,

    /// 间隔倍增因子
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,

    /// 抖动因子（±百分比）
    #[serde(default = "default_jitter")]
    pub jitter: f64,

    /// 最大重试次数（None 表示无限）
    pub max_retries: Option<u32>,
}

fn default_initial_interval() -> u64 {
    1
}
fn default_max_interval() -> u64 {
    30
}
fn default_multiplier() -> f64 {
    2.0
}
fn default_jitter() -> f64 {
    0.25
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_interval: default_initial_interval(),
            max_interval: default_max_interval(),
            multiplier: default_multiplier(),
            jitter: default_jitter(),
            max_retries: None,
        }
    }
}

/// 计算第 n 次重试的等待时间（秒）
pub fn calculate_wait_time(config: &ReconnectConfig, attempt: u32) -> u64 {
    // 指数退避
    let base_interval = config.initial_interval as f64 * config.multiplier.powi(attempt as i32 - 1);

    // 应用上限
    let capped = base_interval.min(config.max_interval as f64);

    // 添加抖动
    let jitter_range = capped * config.jitter;
    let jitter: f64 = rand::thread_rng().gen_range(-jitter_range..=jitter_range);

    let final_interval = (capped + jitter).max(0.0) as u64;
    final_interval.max(1) // 至少等待 1 秒
}

/// 检查是否应该继续重试
pub fn should_retry(config: &ReconnectConfig, attempt: u32) -> bool {
    match config.max_retries {
        Some(max) => attempt <= max,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert_eq!(config.initial_interval, 1);
        assert_eq!(config.max_interval, 30);
    }

    #[test]
    fn test_calculate_wait_time() {
        let config = ReconnectConfig::default();

        // 第一次重试应该在初始间隔附近
        let wait = calculate_wait_time(&config, 1);
        assert!(wait >= 1 && wait <= 2);

        // 第二次应该是第一次的 2 倍左右
        let wait2 = calculate_wait_time(&config, 2);
        assert!(wait2 >= 1 && wait2 <= 5);
    }

    #[test]
    fn test_should_retry() {
        let config = ReconnectConfig::default();
        assert!(should_retry(&config, 100));

        let config = ReconnectConfig {
            max_retries: Some(3),
            ..Default::default()
        };
        assert!(should_retry(&config, 3));
        assert!(!should_retry(&config, 4));
    }
}
