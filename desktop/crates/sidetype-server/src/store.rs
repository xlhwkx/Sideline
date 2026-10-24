//! 设备 token 的存储。磁盘上只保存哈希，不保存 token 本身。

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Default, Serialize, Deserialize)]
struct File {
    /// token 的 SHA-256，十六进制。
    devices: Vec<String>,
}

/// 已配对设备。
pub struct DeviceStore {
    inner: Mutex<File>,
    path: Option<PathBuf>,
}

impl DeviceStore {
    /// 只活在内存里，测试用。
    pub fn memory() -> Self {
        Self {
            inner: Mutex::new(File::default()),
            path: None,
        }
    }

    /// 从磁盘加载。文件不存在时是空的。
    pub fn load(path: PathBuf) -> Self {
        let file = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(file),
            path: Some(path),
        }
    }

    /// 记下一台新设备。
    pub fn insert(&self, token_hash: String) {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !guard.devices.contains(&token_hash) {
            guard.devices.push(token_hash);
        }
        self.persist(&guard);
    }

    /// 这台设备是否还有效。
    pub fn contains(&self, token_hash: &str) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .devices
            .iter()
            .any(|saved| saved == token_hash)
    }

    /// 撤销一台设备。返回是否真的删掉了。
    pub fn revoke(&self, token_hash: &str) -> bool {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let before = guard.devices.len();
        guard.devices.retain(|saved| saved != token_hash);
        let removed = guard.devices.len() != before;
        if removed {
            self.persist(&guard);
        }
        removed
    }

    /// 全部设备的哈希。
    pub fn list(&self) -> Vec<String> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .devices
            .clone()
    }

    fn persist(&self, file: &File) {
        let Some(path) = &self.path else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(file) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// 生成一个 token 和它的哈希。token 只返回这一次。
pub fn new_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| bytes.fill(0));
    let token = hex(&bytes);
    let hash = hash_token(&token);
    (token, hash)
}

/// token 的 SHA-256，十六进制。
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex(&digest)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0F) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_not_stored_in_plaintext() {
        let (token, hash) = new_token();
        assert_eq!(token.len(), 64);
        assert_ne!(token, hash);
        let store = DeviceStore::memory();
        store.insert(hash.clone());
        assert!(store.contains(&hash_token(&token)));
        assert!(!store.list().iter().any(|saved| saved == &token));
    }

    #[test]
    fn revoked_token_stops_matching() {
        let store = DeviceStore::memory();
        let (_token, hash) = new_token();
        store.insert(hash.clone());
        assert!(store.revoke(&hash));
        assert!(!store.contains(&hash));
        assert!(!store.revoke(&hash));
    }
}
