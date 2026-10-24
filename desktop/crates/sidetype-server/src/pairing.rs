//! 一次性配对码。同一时间只有一个，60 秒过期，关闭窗口立即作废。

use std::sync::Mutex;
use std::time::Instant;

use sidetype_protocol::PAIR_CODE_TTL;

/// 默认端口。
pub const PAIR_PORT: u16 = 17331;

struct Issued {
    code: String,
    expires: Instant,
}

/// 配对码的持有者。
pub struct Pairing {
    current: Mutex<Option<Issued>>,
}

impl Pairing {
    /// 空的配对状态。
    pub fn new() -> Self {
        Self {
            current: Mutex::new(None),
        }
    }

    /// 生成一个新的 6 位配对码，旧的立即失效。
    ///
    /// 二维码里带的也是这一份。手机扫到就直接连接，不用再手填。
    pub fn issue(&self) -> String {
        let code = random_code();
        let issued = Issued {
            code: code.clone(),
            expires: Instant::now() + PAIR_CODE_TTL,
        };
        *self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(issued);
        code
    }

    /// 还没过期的配对码。
    pub fn current(&self) -> Option<String> {
        let guard = self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard
            .as_ref()
            .filter(|issued| issued.expires > Instant::now())
            .map(|issued| issued.code.clone())
    }

    /// 作废当前配对码。
    pub fn clear(&self) {
        *self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    /// 配对码是否正确且未过期。比较是恒定时间的。
    pub fn check(&self, code: &str) -> bool {
        let guard = self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match guard.as_ref() {
            Some(issued) if issued.expires > Instant::now() => {
                constant_eq(issued.code.as_bytes(), code.as_bytes())
            }
            _ => false,
        }
    }
}

impl Default for Pairing {
    fn default() -> Self {
        Self::new()
    }
}

fn random_code() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| bytes.fill(0));
    let value = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{value:06}")
}

fn constant_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_six_digits_and_single_use_window() {
        let pairing = Pairing::new();
        let first = pairing.issue();
        assert_eq!(first.len(), 6);
        assert!(first.chars().all(|ch| ch.is_ascii_digit()));
        assert!(pairing.check(&first));

        let second = pairing.issue();
        assert!(!pairing.check(&first));
        assert!(pairing.check(&second));

        pairing.clear();
        assert!(!pairing.check(&second));
    }

    #[test]
    fn wrong_length_never_matches() {
        let pairing = Pairing::new();
        pairing.issue();
        assert!(!pairing.check("12345"));
        assert!(!pairing.check("1234567"));
    }
}
