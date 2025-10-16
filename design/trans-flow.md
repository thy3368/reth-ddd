# 以太坊交易完整生命周期

从执行客户端视角详细描述一笔转账交易从创建、提交、执行到最终确认的完整生命周期。

## 目录
- [1. 概述](#1-概述)
- [2. 交易创建与签名 (客户端侧)](#2-交易创建与签名-客户端侧)
- [3. 交易提交 (JSON-RPC)](#3-交易提交-json-rpc)
- [4. 交易验证与池管理](#4-交易验证与池管理)
- [5. P2P 网络广播](#5-p2p-网络广播)
- [6. 区块构建 (Engine API)](#6-区块构建-engine-api)
- [7. 交易执行 (EVM)](#7-交易执行-evm)
- [8. 状态更新与持久化](#8-状态更新与持久化)
- [9. 区块传播与确认](#9-区块传播与确认)
- [10. 最终确认 (Finality)](#10-最终确认-finality)
- [11. 完整时序图](#11-完整时序图)
- [12. 性能关键路径优化](#12-性能关键路径优化)

---

## 1. 概述

### 1.1 交易生命周期阶段

```
┌─────────────────────────────────────────────────────────────────┐
│                      交易完整生命周期                              │
└─────────────────────────────────────────────────────────────────┘

阶段1: 创建与签名 (0-100ms)
  ├─ 用户发起转账请求
  ├─ 钱包构造交易对象
  ├─ 使用私钥签名
  └─ 生成已签名交易 (RLP 编码)

阶段2: 提交 (100-200ms)
  ├─ 通过 JSON-RPC 发送到执行客户端
  ├─ eth_sendRawTransaction
  └─ 返回交易哈希

阶段3: 验证 (200-250ms)
  ├─ 解码和签名验证
  ├─ 基本有效性检查
  ├─ Nonce 检查
  └─ 余额检查

阶段4: 交易池 (250-300ms)
  ├─ 添加到 pending 队列
  ├─ 按 gas 价格排序
  └─ 等待区块构建器选择

阶段5: P2P 广播 (300-500ms)
  ├─ 广播到连接的 peers
  ├─ 传播到全网节点
  └─ 其他节点验证并加入交易池

阶段6: 区块构建 (12秒周期)
  ├─ 共识客户端请求构建 payload
  ├─ 执行客户端从交易池选择交易
  ├─ 构建执行载荷
  └─ 返回 payload 给共识客户端

阶段7: 交易执行 (12秒周期)
  ├─ 共识客户端提交 newPayload
  ├─ EVM 执行交易
  ├─ 状态更新 (余额转移)
  └─ 生成交易收据

阶段8: 状态持久化 (12秒周期)
  ├─ 计算新状态根
  ├─ 验证状态转换
  └─ 持久化到数据库

阶段9: 区块传播 (12-15秒)
  ├─ 共识客户端广播区块
  ├─ 其他节点接收并验证
  └─ Forkchoice 更新

阶段10: 最终确认 (64-95秒)
  ├─ 2 个 epoch (64秒) 后标记为 justified
  ├─ 再 1 个 epoch (32秒) 后标记为 finalized
  └─ 交易不可逆
```

### 1.2 关键时间指标

| 阶段 | 时间 | 说明 |
|------|------|------|
| 交易提交 | < 200ms | 从发起到进入交易池 |
| 网络传播 | 200-500ms | 广播到全网节点 |
| 打包进区块 | 0-12秒 | 等待下一个 slot |
| 区块确认 | 12秒 | 区块被提议和认证 |
| 安全确认 | 64秒 | 2 个 epoch，justified |
| 最终确认 | 96秒 | 3 个 epoch，finalized |

---

## 2. 交易创建与签名 (客户端侧)

### 2.1 用例：Alice 向 Bob 转账 1 ETH

**场景**:
- Alice 地址: `0xAlice...`
- Bob 地址: `0xBob...`
- 转账金额: 1 ETH (1,000,000,000,000,000,000 Wei)
- 当前网络: Mainnet (chainId = 1)

### 2.2 交易对象构造

钱包软件（如 MetaMask）构造交易对象：

```typescript
// 客户端 (钱包) 代码
const transaction = {
  // EIP-1559 交易类型
  type: 2,

  // 链 ID (防止重放攻击)
  chainId: 1,

  // Nonce (Alice 发送的交易序号)
  nonce: 42,

  // Gas 相关参数
  gasLimit: 21000,                      // 标准转账 gas
  maxFeePerGas: 50_000_000_000,        // 50 Gwei
  maxPriorityFeePerGas: 2_000_000_000, // 2 Gwei (给验证者的小费)

  // 转账信息
  to: '0xBob...',
  value: '1000000000000000000',        // 1 ETH in Wei
  data: '0x',                          // 空数据 (简单转账)
};
```

### 2.3 交易签名

使用 Alice 的私钥对交易进行签名：

```typescript
// 1. 计算交易哈希
const txHash = keccak256(rlp.encode([
  chainId,
  nonce,
  maxPriorityFeePerGas,
  maxFeePerGas,
  gasLimit,
  to,
  value,
  data,
  [] // accessList (空)
]));

// 2. 使用 ECDSA 签名
const signature = secp256k1.sign(txHash, alicePrivateKey);

// 3. 构造完整的已签名交易
const signedTx = {
  ...transaction,
  v: signature.v,
  r: signature.r,
  s: signature.s,
};

// 4. RLP 编码
const rawTx = rlp.encode([
  chainId,
  nonce,
  maxPriorityFeePerGas,
  maxFeePerGas,
  gasLimit,
  to,
  value,
  data,
  [], // accessList
  v,
  r,
  s
]);

// 5. 得到十六进制字符串
const rawTxHex = '0x' + rawTx.toString('hex');
// 例如: "0x02f876018229...a04ba69724e8f69de52f0125ad8b3c5c2cef33019"
```

**关键点**:
- **Nonce**: 必须等于 Alice 账户当前的交易计数，防止重放攻击
- **Gas 参数**: `maxFeePerGas` 必须 ≥ 当前区块的 `baseFeePerGas`
- **签名**: 使用 secp256k1 椭圆曲线，生成 `v`, `r`, `s` 三个值
- **RLP 编码**: 按照 EIP-1559 格式编码，类型前缀 `0x02`

---

## 3. 交易提交 (JSON-RPC)

### 3.1 通过 JSON-RPC 提交

钱包将已签名交易发送到执行客户端（如 Geth、Reth）：

```json
POST https://mainnet.infura.io/v3/YOUR-PROJECT-ID

{
  "jsonrpc": "2.0",
  "method": "eth_sendRawTransaction",
  "params": [
    "0x02f876018229...a04ba69724e8f69de52f0125ad8b3c5c2cef33019"
  ],
  "id": 1
}
```

### 3.2 执行客户端接收处理

```rust
// interfaces/rpc/handlers/eth.rs
pub struct EthApiHandler {
    submit_tx_usecase: Arc<SubmitRawTransactionUseCase>,
}

#[async_trait]
impl EthApiServer for EthApiHandler {
    async fn send_raw_transaction(&self, raw_tx: Bytes) -> RpcResult<H256> {
        // 调用用例层
        let tx_hash = self.submit_tx_usecase
            .execute(raw_tx)
            .await
            .map_err(|e| convert_error(e))?;

        // 返回交易哈希
        Ok(tx_hash)
    }
}
```

### 3.3 返回交易哈希

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
}
```

钱包显示：
```
✓ Transaction submitted!
Transaction Hash: 0x88df...944b
View on Etherscan: https://etherscan.io/tx/0x88df...944b
```

**时间消耗**: 约 50-100ms（网络往返时间）

---

## 4. 交易验证与池管理

### 4.1 交易解码

```rust
// domain/usecases/submit_transaction.rs
pub struct SubmitRawTransactionUseCase {
    tx_pool: Arc<dyn TransactionPool>,
    tx_validator: Arc<dyn TransactionValidator>,
    p2p_service: Arc<dyn P2PService>,
    state_db: Arc<dyn StateRepository>,
}

impl SubmitRawTransactionUseCase {
    pub async fn execute(&self, raw_tx: Bytes) -> Result<TxHash, UseCaseError> {
        // === 步骤 1: 解码交易 ===
        let tx = self.decode_transaction(&raw_tx)?;

        // === 步骤 2: 恢复发送者地址 ===
        let sender = self.recover_sender(&tx)?;

        // === 步骤 3: 验证交易 ===
        self.validate_transaction(&tx, sender).await?;

        // === 步骤 4: 添加到交易池 ===
        let tx_hash = self.tx_pool.add(tx.clone()).await?;

        // === 步骤 5: 广播到 P2P 网络 ===
        self.p2p_service.broadcast_transactions(vec![tx]).await?;

        Ok(tx_hash)
    }

    fn decode_transaction(&self, raw_tx: &Bytes) -> Result<Transaction, UseCaseError> {
        // 1. 检查交易类型前缀
        let tx_type = match raw_tx.first() {
            Some(&0x00) => TransactionType::Legacy,
            Some(&0x01) => TransactionType::Eip2930,
            Some(&0x02) => TransactionType::Eip1559,
            Some(&0x03) => TransactionType::Eip4844,
            _ => return Err(UseCaseError::InvalidTransactionType),
        };

        // 2. RLP 解码
        let tx = match tx_type {
            TransactionType::Eip1559 => {
                let decoded: Eip1559Transaction = rlp::decode(&raw_tx[1..])?;
                Transaction::Eip1559(decoded)
            }
            // ... 其他类型
            _ => todo!(),
        };

        Ok(tx)
    }

    fn recover_sender(&self, tx: &Transaction) -> Result<Address, UseCaseError> {
        // 1. 重新计算交易哈希
        let tx_hash = tx.compute_hash();

        // 2. 从签名恢复公钥
        let signature = Signature::from_rsv(tx.r, tx.s, tx.v);
        let public_key = secp256k1::recover(&tx_hash, &signature)
            .map_err(|_| UseCaseError::InvalidSignature)?;

        // 3. 从公钥导出地址 (keccak256(pubkey)[12..32])
        let address = Address::from_public_key(&public_key);

        Ok(address)
    }
}
```

### 4.2 交易验证

```rust
// domain/services/transaction_validator.rs
pub struct TransactionValidatorService {
    chain_id: u64,
    config: ValidationConfig,
}

impl TransactionValidator for TransactionValidatorService {
    async fn validate(
        &self,
        tx: &Transaction,
        sender: Address,
        state: &State,
    ) -> Result<(), ValidationError> {
        // === 验证 1: Chain ID ===
        if tx.chain_id != self.chain_id {
            return Err(ValidationError::InvalidChainId {
                expected: self.chain_id,
                got: tx.chain_id,
            });
        }

        // === 验证 2: Gas Limit ===
        if tx.gas_limit < 21_000 {
            return Err(ValidationError::GasLimitTooLow);
        }

        if tx.gas_limit > 30_000_000 {
            return Err(ValidationError::GasLimitTooHigh);
        }

        // === 验证 3: Gas Price ===
        match tx.transaction_type {
            TransactionType::Eip1559 => {
                // EIP-1559: 检查 maxFeePerGas
                if tx.max_fee_per_gas < 1_000_000_000 {  // 最低 1 Gwei
                    return Err(ValidationError::GasPriceTooLow);
                }

                // maxPriorityFeePerGas 不能超过 maxFeePerGas
                if tx.max_priority_fee_per_gas > tx.max_fee_per_gas {
                    return Err(ValidationError::PriorityFeeExceedsMaxFee);
                }
            }
            TransactionType::Legacy => {
                if tx.gas_price < 1_000_000_000 {
                    return Err(ValidationError::GasPriceTooLow);
                }
            }
            _ => {}
        }

        // === 验证 4: Nonce ===
        let account = state.get_account(sender).await?;
        let current_nonce = account.map(|a| a.nonce).unwrap_or(0);

        if tx.nonce < current_nonce {
            return Err(ValidationError::NonceTooLow {
                got: tx.nonce,
                expected: current_nonce,
            });
        }

        // Nonce gap 检查（不能跳过 nonce）
        if tx.nonce > current_nonce + self.config.max_nonce_gap {
            return Err(ValidationError::NonceGapTooLarge);
        }

        // === 验证 5: 余额检查 ===
        let balance = account.map(|a| a.balance).unwrap_or(U256::ZERO);

        // 计算最大可能花费
        let max_cost = tx.value + U256::from(tx.gas_limit) * tx.max_fee_per_gas;

        if balance < max_cost {
            return Err(ValidationError::InsufficientFunds {
                required: max_cost,
                available: balance,
            });
        }

        // === 验证 6: 签名检查 ===
        if !self.verify_signature(tx, sender) {
            return Err(ValidationError::InvalidSignature);
        }

        // === 验证 7: 数据大小限制 ===
        if tx.data.len() > self.config.max_data_size {
            return Err(ValidationError::DataTooLarge);
        }

        Ok(())
    }
}
```

### 4.3 添加到交易池

```rust
// infrastructure/tx_pool/service.rs
pub struct TransactionPoolService {
    // Pending: 可立即执行的交易 (nonce 连续)
    pending: Arc<RwLock<BTreeMap<Address, Vec<Transaction>>>>,

    // Queued: 等待前序交易的交易 (nonce 有间隙)
    queued: Arc<RwLock<BTreeMap<Address, Vec<Transaction>>>>,

    // 交易索引 (快速查找)
    by_hash: Arc<RwLock<HashMap<TxHash, Transaction>>>,

    state_db: Arc<dyn StateRepository>,
    config: TxPoolConfig,
}

impl TransactionPool for TransactionPoolService {
    async fn add(&self, tx: Transaction) -> Result<TxHash, Error> {
        let sender = tx.recover_sender().unwrap();
        let tx_hash = tx.hash();

        // === 检查是否已存在 ===
        {
            let by_hash = self.by_hash.read().await;
            if by_hash.contains_key(&tx_hash) {
                return Err(Error::TransactionAlreadyExists);
            }
        }

        // === 获取账户当前 nonce ===
        let account_nonce = self.get_account_nonce(sender).await?;

        let mut pending = self.pending.write().await;
        let mut queued = self.queued.write().await;

        if tx.nonce == account_nonce {
            // === Case 1: Nonce 匹配，直接加入 pending ===
            pending
                .entry(sender)
                .or_insert_with(Vec::new)
                .push(tx.clone());

            // 按 nonce 排序
            if let Some(txs) = pending.get_mut(&sender) {
                txs.sort_by_key(|t| t.nonce);
            }

            // 尝试提升 queued 中的后续交易
            self.promote_transactions(sender, &mut pending, &mut queued, account_nonce).await?;

        } else if tx.nonce > account_nonce {
            // === Case 2: Nonce 有间隙，加入 queued ===
            queued
                .entry(sender)
                .or_insert_with(Vec::new)
                .push(tx.clone());

            if let Some(txs) = queued.get_mut(&sender) {
                txs.sort_by_key(|t| t.nonce);
            }

        } else {
            // === Case 3: Nonce 过低，拒绝 ===
            return Err(Error::NonceTooLow);
        }

        // === 更新索引 ===
        {
            let mut by_hash = self.by_hash.write().await;
            by_hash.insert(tx_hash, tx);
        }

        // === 检查交易池限制 ===
        self.enforce_limits(&mut pending, &mut queued).await?;

        Ok(tx_hash)
    }

    async fn promote_transactions(
        &self,
        sender: Address,
        pending: &mut BTreeMap<Address, Vec<Transaction>>,
        queued: &mut BTreeMap<Address, Vec<Transaction>>,
        mut next_nonce: u64,
    ) -> Result<(), Error> {
        // 从 queued 中提升可执行的交易到 pending
        if let Some(queued_txs) = queued.get_mut(&sender) {
            let mut to_promote = Vec::new();

            for tx in queued_txs.iter() {
                if tx.nonce == next_nonce + 1 {
                    to_promote.push(tx.clone());
                    next_nonce += 1;
                } else {
                    break;  // Nonce 不连续，停止
                }
            }

            // 移除已提升的交易
            queued_txs.drain(0..to_promote.len());

            // 添加到 pending
            if !to_promote.is_empty() {
                pending
                    .entry(sender)
                    .or_insert_with(Vec::new)
                    .extend(to_promote);
            }
        }

        Ok(())
    }

    async fn enforce_limits(
        &self,
        pending: &mut BTreeMap<Address, Vec<Transaction>>,
        queued: &mut BTreeMap<Address, Vec<Transaction>>,
    ) -> Result<(), Error> {
        // === 限制 1: 总交易数 ===
        let total_pending: usize = pending.values().map(|v| v.len()).sum();
        let total_queued: usize = queued.values().map(|v| v.len()).sum();

        if total_pending + total_queued > self.config.max_pool_size {
            // 移除价格最低的 queued 交易
            self.evict_lowest_priced(queued, total_pending + total_queued - self.config.max_pool_size);
        }

        // === 限制 2: 单账户交易数 ===
        for (address, txs) in pending.iter_mut() {
            if txs.len() > self.config.max_per_account {
                txs.truncate(self.config.max_per_account);
            }
        }

        Ok(())
    }
}
```

**时间消耗**: 约 10-50ms

---

## 5. P2P 网络广播

### 5.1 广播到直连节点

```rust
// infrastructure/p2p/transaction_propagation.rs
pub struct P2PTransactionPropagation {
    peer_manager: Arc<dyn PeerManager>,
    config: PropagationConfig,
}

impl P2PTransactionPropagation {
    pub async fn broadcast_transactions(
        &self,
        txs: Vec<Transaction>,
    ) -> Result<(), Error> {
        // === 获取活跃的 peers ===
        let peers = self.peer_manager.get_active_peers().await?;

        if peers.is_empty() {
            return Err(Error::NoPeersAvailable);
        }

        // === 策略: 随机选择 sqrt(n) 个 peers ===
        let broadcast_count = (peers.len() as f64).sqrt().ceil() as usize;
        let selected_peers = self.select_random_peers(&peers, broadcast_count);

        // === 并行广播 ===
        let broadcast_futures = selected_peers.into_iter().map(|peer| {
            let txs = txs.clone();
            async move {
                self.send_to_peer(peer, txs).await
            }
        });

        // 并发执行，忽略单个失败
        let results = futures::future::join_all(broadcast_futures).await;

        let success_count = results.iter().filter(|r| r.is_ok()).count();

        tracing::info!(
            "Broadcasted {} transactions to {}/{} peers",
            txs.len(),
            success_count,
            results.len()
        );

        Ok(())
    }

    async fn send_to_peer(
        &self,
        peer: PeerId,
        txs: Vec<Transaction>,
    ) -> Result<(), Error> {
        // === 过滤 peer 已知的交易 ===
        let unknown_txs = txs.into_iter()
            .filter(|tx| !self.peer_manager.peer_has_transaction(peer, tx.hash()))
            .collect::<Vec<_>>();

        if unknown_txs.is_empty() {
            return Ok(());
        }

        // === 构造 eth/68 Transactions 消息 ===
        let message = EthMessage::Transactions {
            transactions: unknown_txs.iter().map(|tx| tx.encode_signed()).collect(),
        };

        // === 发送消息 ===
        self.peer_manager.send_message(peer, message).await?;

        // === 标记 peer 已知这些交易 ===
        for tx in &unknown_txs {
            self.peer_manager.mark_transaction_known(peer, tx.hash()).await;
        }

        Ok(())
    }

    fn select_random_peers(&self, peers: &[PeerId], count: usize) -> Vec<PeerId> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();

        let mut selected = peers.to_vec();
        selected.shuffle(&mut rng);
        selected.truncate(count);
        selected
    }
}
```

### 5.2 网络传播

```
Alice's Node                    Network Topology
     │
     ├─ broadcast to 4 peers ───┬─► Peer A
     │                          │     │
     │                          │     ├─► propagates to 3 peers
     │                          │     └─► reaches 12 nodes
     │                          │
     │                          ├─► Peer B
     │                          │     └─► propagates...
     │                          │
     │                          ├─► Peer C
     │                          │
     │                          └─► Peer D

在 200-500ms 内，交易传播到全网数千个节点
```

### 5.3 接收节点的处理

```rust
// interfaces/p2p/handlers/eth.rs
pub struct EthProtocolHandler {
    tx_pool: Arc<dyn TransactionPool>,
    tx_validator: Arc<dyn TransactionValidator>,
}

impl EthProtocolHandler {
    pub async fn handle_transactions_message(
        &self,
        peer: PeerId,
        message: TransactionsMessage,
    ) -> Result<(), Error> {
        for raw_tx in message.transactions {
            // 1. 解码交易
            let tx = Transaction::decode_signed(&raw_tx)?;
            let tx_hash = tx.hash();

            // 2. 检查是否已知
            if self.tx_pool.contains(tx_hash).await? {
                continue;
            }

            // 3. 验证交易
            match self.tx_validator.validate(&tx).await {
                Ok(_) => {
                    // 4. 添加到交易池
                    if let Ok(_) = self.tx_pool.add(tx.clone()).await {
                        tracing::debug!(
                            "Received transaction {} from peer {}",
                            tx_hash,
                            peer
                        );

                        // 5. 继续传播（但不发回给发送者）
                        self.propagate_to_others(tx, peer).await?;
                    }
                }
                Err(e) => {
                    // 记录但不惩罚 peer（可能是时间差导致的验证失败）
                    tracing::warn!(
                        "Invalid transaction {} from peer {}: {:?}",
                        tx_hash,
                        peer,
                        e
                    );
                }
            }
        }

        Ok(())
    }
}
```

**时间消耗**: 200-500ms（全网传播）

---

## 6. 区块构建 (Engine API)

### 6.1 共识客户端请求构建

每 12 秒一个 slot，共识客户端通过 Engine API 请求执行客户端构建新区块。

```rust
// 时间线示例:
// t=0s: Slot N 开始
// t=4s: 共识客户端调用 engine_forkchoiceUpdatedV3 (带 PayloadAttributes)
// t=4s-8s: 执行客户端后台构建 payload
// t=8s: 共识客户端调用 engine_getPayloadV3 获取构建好的 payload
// t=8s-12s: 共识客户端提议区块并收集认证
```

**Forkchoice 更新请求**:

```json
{
  "jsonrpc": "2.0",
  "method": "engine_forkchoiceUpdatedV3",
  "params": [
    {
      "headBlockHash": "0xParentBlockHash...",
      "safeBlockHash": "0x...",
      "finalizedBlockHash": "0x..."
    },
    {
      "timestamp": "0x65a4e8b0",
      "prevRandao": "0xRandomValue...",
      "suggestedFeeRecipient": "0xValidator...",
      "withdrawals": [
        {
          "index": "0x123",
          "validatorIndex": "0x456",
          "address": "0xWithdrawalAddress...",
          "amount": "0x1000"
        }
      ],
      "parentBeaconBlockRoot": "0xBeaconRoot..."
    }
  ],
  "id": 1
}
```

### 6.2 执行客户端构建 Payload

```rust
// infrastructure/payload_builder/service.rs
pub struct PayloadBuilderService {
    tx_pool: Arc<dyn TransactionPool>,
    state_db: Arc<dyn StateRepository>,
    tx_executor: Arc<dyn TransactionExecutor>,
    payloads: Arc<RwLock<HashMap<PayloadId, BuiltPayload>>>,
}

impl PayloadBuilder for PayloadBuilderService {
    async fn start_building(
        &self,
        parent: H256,
        attrs: PayloadAttributes,
    ) -> Result<PayloadId, Error> {
        let payload_id = PayloadId::new(&parent, &attrs);

        // === 在后台异步构建 payload ===
        let builder = self.clone();
        tokio::spawn(async move {
            match builder.build_payload(parent, attrs).await {
                Ok(payload) => {
                    let mut payloads = builder.payloads.write().await;
                    payloads.insert(payload_id, payload);

                    tracing::info!(
                        "Built payload {} with {} transactions",
                        payload_id,
                        payload.execution_payload.transactions.len()
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to build payload: {:?}", e);
                }
            }
        });

        Ok(payload_id)
    }

    async fn build_payload(
        &self,
        parent: H256,
        attrs: PayloadAttributes,
    ) -> Result<BuiltPayload, Error> {
        let start_time = Instant::now();

        // === 步骤 1: 获取父区块和状态 ===
        let parent_block = self.state_db.get_block(parent).await?.unwrap();
        let mut state = self.state_db.at_block(parent).await?;

        // === 步骤 2: 计算 base fee ===
        let base_fee = calculate_next_base_fee(
            parent_block.gas_used,
            parent_block.gas_limit,
            parent_block.base_fee_per_gas,
        );

        tracing::info!(
            "Building payload on top of block {}, base_fee={}",
            parent_block.number,
            base_fee
        );

        // === 步骤 3: 从交易池选择交易 ===
        let candidate_txs = self.tx_pool.select_transactions(
            parent_block.gas_limit,
            |tx| self.is_transaction_eligible(tx, base_fee)
        ).await?;

        tracing::info!(
            "Selected {} candidate transactions from pool",
            candidate_txs.len()
        );

        // === 步骤 4: 存储 parent beacon root (EIP-4788) ===
        self.store_beacon_root(&mut state, attrs.parent_beacon_block_root, attrs.timestamp)?;

        // === 步骤 5: 执行交易 ===
        let mut included_txs = Vec::new();
        let mut receipts = Vec::new();
        let mut cumulative_gas = 0u64;
        let mut total_fees = U256::ZERO;
        let mut logs_bloom = Bloom::default();

        for tx in candidate_txs {
            // 检查 gas limit
            if cumulative_gas + tx.gas_limit > parent_block.gas_limit {
                break;
            }

            // 执行交易
            match self.tx_executor.execute(
                &tx,
                &mut state,
                &attrs,
                cumulative_gas,
                base_fee,
            ).await {
                Ok(receipt) => {
                    cumulative_gas += receipt.gas_used;

                    // 计算实际支付的费用
                    let effective_gas_price = calculate_effective_gas_price(&tx, base_fee);
                    total_fees += U256::from(receipt.gas_used) * effective_gas_price;

                    logs_bloom.accrue_bloom(&receipt.logs_bloom);

                    included_txs.push(tx.encode_signed());
                    receipts.push(receipt);

                    tracing::trace!(
                        "Included tx {} (gas_used={})",
                        tx.hash(),
                        receipt.gas_used
                    );
                }
                Err(e) => {
                    // 交易执行失败，跳过并继续
                    tracing::warn!(
                        "Transaction {} execution failed: {:?}, skipping",
                        tx.hash(),
                        e
                    );
                    continue;
                }
            }
        }

        // === 步骤 6: 处理提款 (Withdrawals) ===
        for withdrawal in &attrs.withdrawals {
            state.add_balance(
                withdrawal.address,
                U256::from(withdrawal.amount) * U256::from(1_000_000_000), // Gwei to Wei
            )?;

            tracing::trace!(
                "Processed withdrawal: validator={} address={} amount={} Gwei",
                withdrawal.validator_index,
                withdrawal.address,
                withdrawal.amount
            );
        }

        // === 步骤 7: 计算状态根 ===
        let state_root = state.compute_root()?;
        let receipts_root = calculate_receipts_root(&receipts);
        let withdrawals_root = calculate_withdrawals_root(&attrs.withdrawals);
        let transactions_root = calculate_transactions_root(&included_txs);

        // === 步骤 8: 计算 blob gas (EIP-4844) ===
        let blob_gas_used = calculate_blob_gas_used(&receipts);
        let excess_blob_gas = calculate_excess_blob_gas(
            parent_block.excess_blob_gas.unwrap_or(0),
            parent_block.blob_gas_used.unwrap_or(0),
        );

        // === 步骤 9: 构建 Execution Payload ===
        let payload = ExecutionPayload {
            parent_hash: parent,
            fee_recipient: attrs.suggested_fee_recipient,
            state_root,
            receipts_root,
            logs_bloom,
            prev_randao: attrs.prev_randao,
            block_number: parent_block.number + 1,
            gas_limit: parent_block.gas_limit,
            gas_used: cumulative_gas,
            timestamp: attrs.timestamp,
            extra_data: Bytes::new(),
            base_fee_per_gas: base_fee,
            block_hash: H256::zero(), // 稍后计算
            transactions: included_txs,
            withdrawals: attrs.withdrawals,
            blob_gas_used: Some(blob_gas_used),
            excess_blob_gas: Some(excess_blob_gas),
        };

        // === 步骤 10: 计算区块哈希 ===
        let block_hash = payload.compute_block_hash();
        let payload = ExecutionPayload { block_hash, ..payload };

        let build_time = start_time.elapsed();

        tracing::info!(
            "Payload built in {:?}: {} txs, {} gas used, {} ETH fees",
            build_time,
            payload.transactions.len(),
            cumulative_gas,
            total_fees / U256::from(1_000_000_000_000_000_000u64) // Wei to ETH
        );

        Ok(BuiltPayload {
            execution_payload: payload,
            block_value: total_fees,
            blobs_bundle: None, // 如果有 blob transactions，填充此字段
        })
    }

    fn is_transaction_eligible(&self, tx: &Transaction, base_fee: U256) -> bool {
        match tx.transaction_type {
            TransactionType::Legacy => {
                tx.gas_price >= base_fee
            }
            TransactionType::Eip1559 | TransactionType::Eip4844 => {
                tx.max_fee_per_gas >= base_fee
            }
            _ => true,
        }
    }
}

// === Base Fee 计算 (EIP-1559) ===
fn calculate_next_base_fee(
    parent_gas_used: u64,
    parent_gas_limit: u64,
    parent_base_fee: U256,
) -> U256 {
    let parent_gas_target = parent_gas_limit / 2;

    if parent_gas_used == parent_gas_target {
        // Gas 使用刚好等于目标，base fee 不变
        return parent_base_fee;
    }

    if parent_gas_used > parent_gas_target {
        // Gas 使用超过目标，增加 base fee
        let gas_used_delta = parent_gas_used - parent_gas_target;
        let base_fee_delta = std::cmp::max(
            parent_base_fee * U256::from(gas_used_delta) / U256::from(parent_gas_target) / U256::from(8),
            U256::from(1),
        );
        parent_base_fee + base_fee_delta
    } else {
        // Gas 使用低于目标，降低 base fee
        let gas_used_delta = parent_gas_target - parent_gas_used;
        let base_fee_delta = parent_base_fee * U256::from(gas_used_delta) / U256::from(parent_gas_target) / U256::from(8);
        parent_base_fee.saturating_sub(base_fee_delta)
    }
}
```

### 6.3 返回 Payload

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "payloadStatus": {
      "status": "VALID",
      "latestValidHash": "0xParentBlockHash...",
      "validationError": null
    },
    "payloadId": "0x1234567890abcdef"
  }
}
```

共识客户端稍后调用 `engine_getPayloadV3` 获取完整的 payload。

**时间消耗**: 后台异步构建，通常 1-4 秒

---

## 7. 交易执行 (EVM)

### 7.1 共识客户端提交 Payload

验证者节点选中后，共识客户端将 payload 提交回执行客户端执行：

```json
{
  "jsonrpc": "2.0",
  "method": "engine_newPayloadV3",
  "params": [
    {
      "parentHash": "0x...",
      "feeRecipient": "0xValidator...",
      "stateRoot": "0x...",
      "receiptsRoot": "0x...",
      "logsBloom": "0x...",
      "prevRandao": "0x...",
      "blockNumber": "0x1234567",
      "gasLimit": "0x1c9c380",
      "gasUsed": "0xabc123",
      "timestamp": "0x65a4e8b0",
      "extraData": "0x",
      "baseFeePerGas": "0x7",
      "blockHash": "0xNewBlockHash...",
      "transactions": [
        "0x02f876...",  // Alice -> Bob 转账交易在这里
        "0x02f877...",
        // ... 其他交易
      ],
      "withdrawals": [...]
    },
    [],  // expectedBlobVersionedHashes
    "0xBeaconRoot..."  // parentBeaconBlockRoot
  ],
  "id": 1
}
```

### 7.2 执行 Alice -> Bob 转账

```rust
// infrastructure/evm/executor.rs
pub struct TransactionExecutorService {
    evm: Arc<dyn EvmExecutor>,
}

impl TransactionExecutor for TransactionExecutorService {
    async fn execute(
        &self,
        tx: &Transaction,
        state: &mut State,
        block_header: &Header,
        cumulative_gas: u64,
        base_fee: U256,
    ) -> Result<TransactionReceipt, ExecutionError> {
        let sender = tx.recover_sender().unwrap();

        tracing::debug!(
            "Executing transaction {} from {} to {:?}",
            tx.hash(),
            sender,
            tx.to
        );

        // === 步骤 1: 增加 nonce ===
        let mut sender_account = state.get_account(sender).await?.unwrap();
        sender_account.nonce += 1;
        state.set_account(sender, sender_account.clone()).await?;

        // === 步骤 2: 扣除最大可能费用 ===
        let max_fee = U256::from(tx.gas_limit) * tx.max_fee_per_gas;
        sender_account.balance = sender_account.balance.checked_sub(tx.value + max_fee)
            .ok_or(ExecutionError::InsufficientFunds)?;
        state.set_account(sender, sender_account.clone()).await?;

        // === 步骤 3: 构建 EVM 环境 ===
        let env = EvmEnv {
            block_number: block_header.number,
            timestamp: block_header.timestamp,
            gas_limit: tx.gas_limit,
            base_fee: block_header.base_fee_per_gas,
            coinbase: block_header.fee_recipient,
            difficulty: U256::ZERO,
            prevrandao: block_header.prev_randao,
            chain_id: 1,
        };

        // === 步骤 4: 执行交易 ===
        let result = if let Some(to) = tx.to {
            // 简单转账或合约调用
            if tx.data.is_empty() {
                // 简单转账: 直接更新余额
                self.transfer(state, sender, to, tx.value).await?;

                ExecutionResult::Success {
                    gas_used: 21_000,  // 标准转账 gas
                    output: Bytes::new(),
                    logs: Vec::new(),
                }
            } else {
                // 合约调用: 使用 EVM 执行
                self.evm.call(sender, to, tx.data.clone(), tx.value, tx.gas_limit, state, env).await?
            }
        } else {
            // 合约创建
            self.evm.create(sender, tx.data.clone(), tx.value, tx.gas_limit, state, env).await?
        };

        // === 步骤 5: 计算实际费用 ===
        let gas_used = match &result {
            ExecutionResult::Success { gas_used, .. } => *gas_used,
            ExecutionResult::Revert { gas_used, .. } => *gas_used,
            ExecutionResult::Halt { gas_used, .. } => *gas_used,
        };

        let effective_gas_price = calculate_effective_gas_price(tx, base_fee);
        let gas_fee = U256::from(gas_used) * effective_gas_price;

        // === 步骤 6: 退还多余的 gas ===
        let refund = max_fee - gas_fee;
        sender_account.balance += refund;
        state.set_account(sender, sender_account).await?;

        // === 步骤 7: 支付费用给区块接收者 ===
        let priority_fee = effective_gas_price.saturating_sub(base_fee);
        let coinbase_reward = U256::from(gas_used) * priority_fee;

        let mut coinbase_account = state.get_account(block_header.fee_recipient).await?.unwrap_or_default();
        coinbase_account.balance += coinbase_reward;
        state.set_account(block_header.fee_recipient, coinbase_account).await?;

        // Base fee 被销毁（不增加任何账户）

        // === 步骤 8: 生成交易收据 ===
        let receipt = TransactionReceipt {
            transaction_hash: tx.hash(),
            transaction_index: 0, // 稍后填充
            block_hash: block_header.hash(),
            block_number: block_header.number,
            from: sender,
            to: tx.to,
            cumulative_gas_used: cumulative_gas + gas_used,
            gas_used,
            contract_address: None,
            logs: match result {
                ExecutionResult::Success { logs, .. } => logs,
                _ => Vec::new(),
            },
            logs_bloom: Bloom::default(), // 稍后填充
            status: match result {
                ExecutionResult::Success { .. } => 1,
                _ => 0,
            },
            effective_gas_price,
            tx_type: tx.transaction_type,
            blob_gas_used: None,
            blob_gas_price: None,
        };

        tracing::info!(
            "Transaction {} executed: status={} gas_used={} fee={} Wei",
            tx.hash(),
            receipt.status,
            gas_used,
            gas_fee
        );

        Ok(receipt)
    }

    async fn transfer(
        &self,
        state: &mut State,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), ExecutionError> {
        if value.is_zero() {
            return Ok(());
        }

        // === 转出 ===
        let mut from_account = state.get_account(from).await?.unwrap();
        from_account.balance = from_account.balance.checked_sub(value)
            .ok_or(ExecutionError::InsufficientFunds)?;
        state.set_account(from, from_account).await?;

        // === 转入 ===
        let mut to_account = state.get_account(to).await?.unwrap_or_default();
        to_account.balance += value;
        state.set_account(to, to_account).await?;

        tracing::debug!("Transferred {} Wei from {} to {}", value, from, to);

        Ok(())
    }
}

fn calculate_effective_gas_price(tx: &Transaction, base_fee: U256) -> U256 {
    match tx.transaction_type {
        TransactionType::Legacy => tx.gas_price,
        TransactionType::Eip1559 | TransactionType::Eip4844 => {
            let priority_fee = std::cmp::min(
                tx.max_priority_fee_per_gas,
                tx.max_fee_per_gas.saturating_sub(base_fee),
            );
            base_fee + priority_fee
        }
        _ => tx.gas_price,
    }
}
```

### 7.3 执行结果

```
Alice 账户变化:
  Nonce: 42 -> 43
  Balance: 10 ETH -> 8.9989 ETH
    - 转账: 1 ETH
    - Gas 费用: 0.0011 ETH (21000 gas × 50 Gwei)

Bob 账户变化:
  Balance: 5 ETH -> 6 ETH
    + 收到: 1 ETH

验证者 (fee_recipient) 账户变化:
  Balance: +0.00042 ETH (21000 gas × 2 Gwei priority fee)

Base Fee 销毁:
  21000 gas × 48 Gwei = 0.001008 ETH (从总供应量中移除)
```

**时间消耗**: 约 10-100μs（简单转账）

---

## 8. 状态更新与持久化

### 8.1 计算状态根

```rust
// infrastructure/state/mpt.rs
impl State {
    pub fn compute_root(&self) -> Result<H256, Error> {
        // === 使用 Merkle Patricia Trie 计算状态根 ===
        let mut trie = MerklePatriciaTrie::new(self.db.clone());

        let changed = self.changed.read().await;

        for (address, account) in changed.iter() {
            let key = keccak256(address.as_bytes());
            let value = rlp::encode(account);
            trie.insert(&key, &value)?;
        }

        let root = trie.root_hash()?;

        tracing::debug!(
            "Computed state root: {} ({} accounts changed)",
            root,
            changed.len()
        );

        Ok(root)
    }
}
```

### 8.2 验证状态根

```rust
// domain/usecases/engine/new_payload.rs
impl NewPayloadUseCase {
    pub async fn execute(
        &self,
        payload: ExecutionPayload,
        // ...
    ) -> Result<PayloadStatus, UseCaseError> {
        // ... 执行所有交易 ...

        // === 验证计算的状态根是否匹配 ===
        let computed_state_root = state.compute_root()?;

        if computed_state_root != payload.state_root {
            tracing::error!(
                "State root mismatch: computed={}, expected={}",
                computed_state_root,
                payload.state_root
            );

            return Ok(PayloadStatus::Invalid {
                latest_valid_hash: Some(payload.parent_hash),
                validation_error: Some("Invalid state root".into()),
            });
        }

        // === 验证通过，持久化状态 ===
        self.state_db.commit(state).await?;

        Ok(PayloadStatus::Valid {
            latest_valid_hash: payload.block_hash,
        })
    }
}
```

### 8.3 持久化到数据库

```rust
// infrastructure/state/database.rs
impl StateRepository for StateDatabase {
    async fn commit(&self, state: State) -> Result<H256, Error> {
        let changed = state.changed.read().await;

        tracing::info!(
            "Committing state: {} accounts, {} storage slots",
            changed.len(),
            changed.values().map(|acc| acc.storage_changes.len()).sum::<usize>()
        );

        // === 批量写入变更 ===
        let mut batch = self.db.batch_write();

        for (address, account) in changed.iter() {
            // 写入账户数据
            let account_key = Self::account_key(address);
            let account_value = rlp::encode(&AccountRlp {
                nonce: account.nonce,
                balance: account.balance,
                storage_root: account.storage_root,
                code_hash: account.code_hash,
            });
            batch.put(&account_key, &account_value);

            // 写入存储变更
            for (slot, value) in &account.storage_changes {
                let storage_key = Self::storage_key(address, slot);
                batch.put(&storage_key, &value.to_be_bytes());
            }

            // 如果有新代码，写入代码
            if let Some(ref code) = account.code {
                let code_key = Self::code_key(&account.code_hash);
                batch.put(&code_key, code);
            }
        }

        // === 执行批量写入 ===
        batch.commit().await?;

        // === 计算并返回新状态根 ===
        let new_root = state.root;

        tracing::info!("State committed: root={}", new_root);

        Ok(new_root)
    }
}
```

**时间消耗**: 约 10-100ms（取决于变更数量）

---

## 9. 区块传播与确认

### 9.1 Forkchoice 更新

共识客户端确认区块后，更新执行客户端的链头：

```json
{
  "jsonrpc": "2.0",
  "method": "engine_forkchoiceUpdatedV3",
  "params": [
    {
      "headBlockHash": "0xNewBlockHash...",    // Alice 的交易在这个区块
      "safeBlockHash": "0xPreviousSafeBlock...",
      "finalizedBlockHash": "0xFinalizedBlock..."
    },
    null  // 不需要构建新 payload
  ],
  "id": 1
}
```

```rust
// domain/usecases/engine/forkchoice_updated.rs
impl ForkchoiceUpdatedUseCase {
    pub async fn execute(
        &self,
        forkchoice: ForkchoiceState,
        attrs: Option<PayloadAttributes>,
    ) -> Result<ForkchoiceUpdatedResponse, UseCaseError> {
        // === 更新链头 ===
        self.chain_state.set_head(forkchoice.head_block_hash).await?;

        tracing::info!(
            "Forkchoice updated: head={} safe={} finalized={}",
            forkchoice.head_block_hash,
            forkchoice.safe_block_hash,
            forkchoice.finalized_block_hash
        );

        // === 通知订阅者 (WebSocket, 交易池等) ===
        self.notify_new_block(forkchoice.head_block_hash).await?;

        // === 从交易池移除已打包的交易 ===
        self.remove_mined_transactions(forkchoice.head_block_hash).await?;

        Ok(ForkchoiceUpdatedResponse {
            payload_status: PayloadStatus::Valid {
                latest_valid_hash: forkchoice.head_block_hash,
            },
            payload_id: None,
        })
    }

    async fn remove_mined_transactions(&self, block_hash: H256) -> Result<(), Error> {
        let block = self.block_repo.get_block_by_hash(block_hash).await?.unwrap();

        let tx_hashes: Vec<_> = block.transactions.iter().map(|tx| tx.hash()).collect();

        self.tx_pool.remove_transactions(&tx_hashes).await?;

        tracing::info!(
            "Removed {} mined transactions from pool",
            tx_hashes.len()
        );

        Ok(())
    }
}
```

### 9.2 P2P 区块传播

```rust
// infrastructure/p2p/block_propagation.rs
pub struct BlockPropagationService {
    peer_manager: Arc<dyn PeerManager>,
    block_repo: Arc<dyn BlockRepository>,
}

impl BlockPropagationService {
    pub async fn propagate_new_block(&self, block_hash: H256) -> Result<(), Error> {
        let block = self.block_repo.get_block_by_hash(block_hash).await?.unwrap();

        let peers = self.peer_manager.get_active_peers().await?;

        // === 策略: sqrt(n) 个 peers 发送完整区块，其余发送哈希 ===
        let full_block_count = (peers.len() as f64).sqrt() as usize;

        let (full_block_peers, hash_only_peers) = peers.split_at(full_block_count);

        // === 发送完整区块 ===
        for peer in full_block_peers {
            let message = EthMessage::NewBlock {
                block: block.clone(),
                td: self.calculate_total_difficulty(&block).await?,
            };
            self.peer_manager.send_message(*peer, message).await?;
        }

        // === 发送区块哈希 ===
        for peer in hash_only_peers {
            let message = EthMessage::NewBlockHashes(vec![
                BlockHashNumber {
                    hash: block_hash,
                    number: block.number,
                }
            ]);
            self.peer_manager.send_message(*peer, message).await?;
        }

        tracing::info!(
            "Propagated block {} to {} peers (full: {}, hash: {})",
            block_hash,
            peers.len(),
            full_block_count,
            hash_only_peers.len()
        );

        Ok(())
    }
}
```

### 9.3 用户查询交易状态

用户可以通过交易哈希查询收据：

```json
{
  "jsonrpc": "2.0",
  "method": "eth_getTransactionReceipt",
  "params": ["0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"],
  "id": 1
}
```

响应：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "transactionHash": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "transactionIndex": "0x0",
    "blockHash": "0xNewBlockHash...",
    "blockNumber": "0x1234567",
    "from": "0xAlice...",
    "to": "0xBob...",
    "cumulativeGasUsed": "0x5208",
    "gasUsed": "0x5208",
    "effectiveGasPrice": "0xba43b7400",
    "contractAddress": null,
    "logs": [],
    "logsBloom": "0x00000000...",
    "status": "0x1",
    "type": "0x2"
  }
}
```

钱包显示：
```
✓ Transaction confirmed!
Block: 19,087,719
Status: Success
Gas Used: 21,000 (100%)
Transaction Fee: 0.00105 ETH
```

---

## 10. 最终确认 (Finality)

### 10.1 以太坊 PoS 最终性机制

```
时间线 (每个 slot = 12秒，每个 epoch = 32 slots = 384秒 ≈ 6.4分钟)

Slot N: Alice 的交易打包进区块
  ├─ 状态: Included
  └─ 可查询: eth_getTransactionReceipt 返回收据

Epoch M (包含 Slot N):
  ├─ 验证者投票认证
  └─ 状态: 1 confirmation

Epoch M+1:
  ├─ 2/3 验证者认证了 Epoch M
  ├─ Epoch M 标记为 "justified"
  └─ 状态: Safe (较难回滚)

Epoch M+2:
  ├─ 2/3 验证者认证了 Epoch M+1
  ├─ Epoch M 标记为 "finalized"
  └─ 状态: Finalized (不可回滚)

时间: Slot N + 64-96秒 = 最终确认
```

### 10.2 查询最终确认状态

```rust
// domain/usecases/get_transaction_finality.rs
pub struct GetTransactionFinalityUseCase {
    block_repo: Arc<dyn BlockRepository>,
    chain_state: Arc<dyn ChainStateRepository>,
}

impl GetTransactionFinalityUseCase {
    pub async fn execute(
        &self,
        tx_hash: H256,
    ) -> Result<FinalityStatus, UseCaseError> {
        // 1. 查找交易所在区块
        let tx = self.block_repo.get_transaction_by_hash(tx_hash).await?
            .ok_or(UseCaseError::TransactionNotFound)?;

        let block = self.block_repo.get_block_by_hash(tx.block_hash).await?
            .ok_or(UseCaseError::BlockNotFound)?;

        // 2. 获取当前链状态
        let latest_block = self.block_repo.get_latest_block().await?;
        let safe_block = self.chain_state.get_safe_block_hash().await?;
        let finalized_block = self.chain_state.get_finalized_block_hash().await?;

        // 3. 判断确认状态
        let confirmations = latest_block.number - block.number;

        let status = if block.hash == finalized_block
            || self.is_ancestor_of(block.hash, finalized_block).await?
        {
            FinalityStatus::Finalized {
                block_number: block.number,
                confirmations,
            }
        } else if block.hash == safe_block
            || self.is_ancestor_of(block.hash, safe_block).await?
        {
            FinalityStatus::Safe {
                block_number: block.number,
                confirmations,
            }
        } else {
            FinalityStatus::Included {
                block_number: block.number,
                confirmations,
            }
        };

        Ok(status)
    }
}

#[derive(Debug)]
pub enum FinalityStatus {
    Included {
        block_number: u64,
        confirmations: u64,
    },
    Safe {
        block_number: u64,
        confirmations: u64,
    },
    Finalized {
        block_number: u64,
        confirmations: u64,
    },
}
```

### 10.3 WebSocket 订阅

用户可以订阅交易确认事件：

```javascript
// 客户端代码
const ws = new WebSocket('wss://mainnet.infura.io/ws/v3/YOUR-PROJECT-ID');

ws.send(JSON.stringify({
  jsonrpc: '2.0',
  method: 'eth_subscribe',
  params: ['newHeads'],
  id: 1
}));

ws.onmessage = (event) => {
  const notification = JSON.parse(event.data);

  if (notification.method === 'eth_subscription') {
    const newBlock = notification.params.result;

    // 检查 Alice 的交易是否在这个区块
    checkTransactionStatus(newBlock.number);
  }
};
```

---

## 11. 完整时序图

```
┌─────────┐  ┌────────┐  ┌──────────────┐  ┌──────────┐  ┌──────────────┐  ┌─────────┐
│ Alice   │  │ Wallet │  │ Execution    │  │ Tx Pool  │  │ Consensus    │  │ Network │
│ (User)  │  │        │  │ Client (EL)  │  │          │  │ Client (CL)  │  │         │
└────┬────┘  └───┬────┘  └──────┬───────┘  └────┬─────┘  └──────┬───────┘  └────┬────┘
     │           │               │               │               │               │
     │ 1. 发起转账│               │               │               │               │
     ├──────────>│               │               │               │               │
     │           │               │               │               │               │
     │           │ 2. 构造交易   │               │               │               │
     │           │    (签名)     │               │               │               │
     │           │               │               │               │               │
     │           │ 3. eth_sendRawTransaction     │               │               │
     │           ├──────────────>│               │               │               │
     │           │               │               │               │               │
     │           │               │ 4. 解码验证   │               │               │
     │           │               │               │               │               │
     │           │               │ 5. 添加到池   │               │               │
     │           │               ├──────────────>│               │               │
     │           │               │               │               │               │
     │           │               │ 6. P2P广播    │               │               │
     │           │               ├───────────────┴───────────────┴──────────────>│
     │           │               │               │               │               │
     │           │ 7. 返回 tx hash│               │               │               │
     │           │<──────────────┤               │               │               │
     │           │               │               │               │               │
     │ 8. 显示确认│               │               │               │               │
     │<──────────┤               │               │               │               │
     │           │               │               │               │               │
     │   --- 等待 12秒 slot ---  │               │               │               │
     │           │               │               │               │               │
     │           │               │               │ 9. forkchoiceUpdated (请求构建)│
     │           │               │<──────────────┴───────────────┤               │
     │           │               │               │               │               │
     │           │               │10. 选择交易   │               │               │
     │           │               │<──────────────┤               │               │
     │           │               │               │               │               │
     │           │               │11. 构建payload│               │               │
     │           │               │ (执行交易)    │               │               │
     │           │               │               │               │               │
     │           │               │12. 返回 payloadId             │               │
     │           │               ├──────────────────────────────>│               │
     │           │               │               │               │               │
     │           │               │13. getPayloadV3│               │               │
     │           │               │<──────────────┴───────────────┤               │
     │           │               │               │               │               │
     │           │               │14. 返回完整payload             │               │
     │           │               ├──────────────────────────────>│               │
     │           │               │               │               │               │
     │           │               │               │15. 提议区块   │               │
     │           │               │               │               ├──────────────>│
     │           │               │               │               │               │
     │           │               │16. newPayloadV3│               │               │
     │           │               │<──────────────┴───────────────┤               │
     │           │               │               │               │               │
     │           │               │17. 验证并执行 │               │               │
     │           │               │               │               │               │
     │           │               │18. 返回 VALID │               │               │
     │           │               ├──────────────────────────────>│               │
     │           │               │               │               │               │
     │           │               │19. forkchoiceUpdated (更新链头)               │
     │           │               │<──────────────┴───────────────┤               │
     │           │               │               │               │               │
     │           │               │20. 更新链头   │               │               │
     │           │               │   从池中移除tx│               │               │
     │           │               │               │               │               │
     │           │               │21. P2P传播区块│               │               │
     │           │               ├───────────────┴───────────────┴──────────────>│
     │           │               │               │               │               │
     │   --- 等待 64-96秒最终确认 ---            │               │               │
     │           │               │               │               │               │
     │22. 查询收据│               │               │               │               │
     ├──────────>│               │               │               │               │
     │           │ eth_getTransactionReceipt     │               │               │
     │           ├──────────────>│               │               │               │
     │           │               │               │               │               │
     │           │   返回收据    │               │               │               │
     │           │<──────────────┤               │               │               │
     │           │               │               │               │               │
     │23. 显示完成│               │               │               │               │
     │<──────────┤               │               │               │               │
     │           │               │               │               │               │
```

---

## 12. 性能关键路径优化

### 12.1 低延迟热路径

```rust
// 关键路径 1: 交易提交
// 目标: < 10ms

#[inline(always)]
pub async fn submit_transaction_hot_path(
    tx: &Transaction,
    tx_pool: &TransactionPool,
) -> Result<TxHash, Error> {
    // 避免不必要的克隆和分配
    let tx_hash = tx.hash_cached();  // 使用缓存的哈希

    // 快速路径检查
    if tx_pool.contains_fast(tx_hash) {
        return Err(Error::AlreadyExists);
    }

    // 无锁添加到池
    tx_pool.add_fast(tx)?;

    Ok(tx_hash)
}
```

### 12.2 并行交易验证

```rust
// 批量验证交易
pub async fn validate_transactions_parallel(
    txs: Vec<Transaction>,
    validator: &TransactionValidator,
) -> Vec<Result<(), ValidationError>> {
    use rayon::prelude::*;

    // 使用 rayon 并行验证
    txs.par_iter()
        .map(|tx| validator.validate_sync(tx))
        .collect()
}
```

### 12.3 状态读取缓存

```rust
// L1 缓存: 线程本地缓存
thread_local! {
    static STATE_CACHE: RefCell<LruCache<Address, Account>> =
        RefCell::new(LruCache::new(1024));
}

pub async fn get_account_cached(
    address: Address,
    state_db: &StateDatabase,
) -> Result<Option<Account>, Error> {
    // 1. 检查线程本地缓存
    let cached = STATE_CACHE.with(|cache| {
        cache.borrow_mut().get(&address).cloned()
    });

    if let Some(account) = cached {
        return Ok(Some(account));
    }

    // 2. 从数据库读取
    let account = state_db.get_account(address).await?;

    // 3. 回填缓存
    if let Some(ref acc) = account {
        STATE_CACHE.with(|cache| {
            cache.borrow_mut().put(address, acc.clone());
        });
    }

    Ok(account)
}
```

### 12.4 零分配序列化

```rust
use bytes::BytesMut;

// 避免堆分配的 RLP 编码
pub fn encode_transaction_zero_alloc(
    tx: &Transaction,
    buf: &mut BytesMut,
) -> Result<(), Error> {
    // 预分配足够的容量
    buf.reserve(256);

    // 直接写入 buffer
    buf.put_u8(tx.transaction_type as u8);
    rlp::encode_to_buf(tx, buf)?;

    Ok(())
}
```

### 12.5 性能监控

```rust
use std::time::Instant;

pub struct PerformanceMetrics {
    tx_submission_time: Histogram,
    tx_validation_time: Histogram,
    tx_execution_time: Histogram,
    state_commit_time: Histogram,
}

impl PerformanceMetrics {
    pub fn record_transaction_flow(&self, tx_hash: H256) {
        let start = Instant::now();

        // ... 执行交易 ...

        let duration = start.elapsed();

        if duration.as_millis() > 100 {
            tracing::warn!(
                "Slow transaction processing: {} took {:?}",
                tx_hash,
                duration
            );
        }

        self.tx_execution_time.record(duration);
    }
}
```

---

## 总结

### 完整生命周期回顾

1. **创建与签名** (0-100ms): 钱包构造交易并使用私钥签名
2. **提交** (100-200ms): 通过 `eth_sendRawTransaction` 发送到执行客户端
3. **验证** (200-250ms): 解码、签名验证、余额检查
4. **交易池** (250-300ms): 添加到 pending 队列
5. **P2P 广播** (300-500ms): 传播到全网节点
6. **区块构建** (0-12秒): 等待下一个 slot，执行客户端构建 payload
7. **交易执行** (12秒): EVM 执行交易，更新状态
8. **状态持久化** (12秒+100ms): 验证状态根并写入数据库
9. **区块传播** (12-15秒): 共识客户端广播区块
10. **最终确认** (64-96秒): 经过 2-3 个 epoch 后不可逆

### 关键性能指标

| 指标 | 目标 | 实际 |
|------|------|------|
| 交易提交响应 | < 100ms | 50-100ms |
| 交易验证 | < 50ms | 10-50ms |
| 简单转账执行 | < 100μs | 10-100μs |
| 状态持久化 | < 100ms | 10-100ms |
| 全网传播 | < 500ms | 200-500ms |
| 打包进区块 | 0-12秒 | 平均 6秒 |
| 最终确认 | 64-96秒 | 约 80秒 |

### 架构特点

- **Clean Architecture**: 领域层、应用层、接口层、基础设施层清晰分离
- **低延迟优化**: 缓存、批量操作、零分配、并行执行
- **容错性**: 交易验证失败不影响其他交易，网络分区自动恢复
- **可观测性**: 完整的日志追踪和性能指标

---

**参考资料**:
- [Ethereum Specification](https://github.com/ethereum/execution-specs)
- [Engine API Specification](https://github.com/ethereum/execution-apis/tree/main/src/engine)
- [EIP-1559: Fee Market](https://eips.ethereum.org/EIPS/eip-1559)
- [EIP-4844: Blob Transactions](https://eips.ethereum.org/EIPS/eip-4844)
- [Gasper: Combining GHOST and Casper](https://arxiv.org/abs/2003.03052)

**文档版本**: v1.0.0
**最后更新**: 2025-10-16
