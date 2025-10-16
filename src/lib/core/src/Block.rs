use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub data: String,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,  // 用于工作量证明的随机数
    pub bits: usize, // 难度目标（前导零数量）
}

impl Block {
    pub fn new(index: u64, data: String, previous_hash: String, bits: usize) -> Self {
        let timestamp = Utc::now().timestamp();
        let mut block = Block {
            index,
            timestamp,
            data,
            previous_hash,
            hash: String::new(),
            nonce: 0,
            bits,
        };
        block.hash = block.calculate_hash();
        block
    }

    pub fn mine_block(&mut self) {
        let prefix = "0".repeat(self.bits); // 目标前缀（如 "00000000"）
        loop {
            self.nonce += 1;
            self.hash = self.calculate_hash();
            if self.hash.starts_with(&prefix) {
                break; // 找到有效哈希
            }
        }
    }

    pub fn calculate_hash(&self) -> String {
        let input = format!(
            "{}{}{}{}{}",
            self.index, self.timestamp, self.data, self.previous_hash, self.nonce
        );
        let mut hasher = Sha256::new();
        hasher.update(input);
        // 将字节数组转换为十六进制字符串
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
