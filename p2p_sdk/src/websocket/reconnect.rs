//! 重连逻辑
//!
//! 实现指数退避重连策略

use super::config::ReconnectConfig;

/// 重连状态跟踪器
#[derive(Debug)]
pub struct ReconnectState {
    /// 重连配置
    config: ReconnectConfig,
    /// 当前重试次数
    attempt: u32,
    /// 是否正在重连
    is_reconnecting: bool,
}

impl ReconnectState {
    /// 创建新的重连状态
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            config,
            attempt: 0,
            is_reconnecting: false,
        }
    }

    /// 开始重连
    pub fn start(&mut self) -> Option<ReconnectAttempt> {
        self.attempt += 1;
        self.is_reconnecting = true;

        if !self.config.should_retry(self.attempt) {
            self.is_reconnecting = false;
            return None;
        }

        Some(ReconnectAttempt {
            attempt: self.attempt,
            wait_seconds: self.config.calculate_wait(self.attempt),
        })
    }

    /// 重连成功
    pub fn succeed(&mut self) {
        self.attempt = 0;
        self.is_reconnecting = false;
    }

    /// 重连失败
    pub fn fail(&mut self) {
        self.is_reconnecting = false;
    }

    /// 重置状态
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.is_reconnecting = false;
    }

    /// 获取当前重试次数
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// 是否正在重连
    pub fn is_reconnecting(&self) -> bool {
        self.is_reconnecting
    }
}

/// 重连尝试信息
#[derive(Debug, Clone)]
pub struct ReconnectAttempt {
    /// 当前重试次数
    pub attempt: u32,
    /// 等待时间（秒）
    pub wait_seconds: u64,
}

impl std::fmt::Display for ReconnectAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "第 {} 次重连尝试，等待 {} 秒",
            self.attempt, self.wait_seconds
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_state_start() {
        let config = ReconnectConfig::default();
        let mut state = ReconnectState::new(config);

        let attempt = state.start();
        assert!(attempt.is_some());
        assert_eq!(attempt.unwrap().attempt, 1);
    }

    #[test]
    fn test_reconnect_state_succeed() {
        let config = ReconnectConfig::default();
        let mut state = ReconnectState::new(config);

        state.start();
        state.succeed();

        assert_eq!(state.attempt(), 0);
        assert!(!state.is_reconnecting());
    }

    #[test]
    fn test_reconnect_state_max_retries() {
        let config = ReconnectConfig {
            max_retries: Some(2),
            ..Default::default()
        };
        let mut state = ReconnectState::new(config);

        assert!(state.start().is_some()); // attempt 1
        state.fail();
        assert!(state.start().is_some()); // attempt 2
        state.fail();
        assert!(state.start().is_none()); // attempt 3 - 超过最大次数
    }

    #[test]
    fn test_reconnect_state_reset() {
        let config = ReconnectConfig::default();
        let mut state = ReconnectState::new(config);

        state.start();
        state.start();
        state.reset();

        assert_eq!(state.attempt(), 0);
    }
}
