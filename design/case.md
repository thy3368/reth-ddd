# 执行客户端与节点交互用例

基于以太坊协议规范的执行客户端（Execution Client）与各类节点的交互用例设计文档。

## 目录
- [1. 概述](#1-概述)
- [2. 与共识客户端交互（Engine API）](#2-与共识客户端交互engine-api)
- [3. P2P网络节点交互](#3-p2p网络节点交互)
- [4. JSON-RPC客户端交互](#4-json-rpc客户端交互)
- [5. 内部组件交互](#5-内部组件交互)
- [6. 用例实现架构](#6-用例实现架构)

---

## 1. 概述

### 1.1 执行客户端职责

执行客户端（如Geth、Reth）在以太坊网络中负责：
- **交易执行**: 执行EVM字节码，更新世界状态
- **状态管理**: 维护账户状态、存储、代码
- **交易池管理**: 管理待处理交易队列
- **区块验证**: 验证区块头、交易、状态根
- **P2P网络通信**: 与其他执行客户端同步数据
- **Engine API**: 与共识客户端协作构建区块

### 1.2 交互层次

```
┌─────────────────────────────────────────────────┐
│           共识客户端 (Consensus Client)           │
│         (Lighthouse, Prysm, Teku, etc.)        │
└────────────────┬────────────────────────────────┘
                 │ Engine API (HTTP/JSON-RPC)
                 │ - engine_newPayloadV3
                 │ - engine_forkchoiceUpdatedV3
                 │ - engine_getPayloadV3
┌────────────────▼────────────────────────────────┐
│          执行客户端 (Execution Client)            │
│              (Geth, Reth, etc.)                │
├─────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │
│  │ 交易池   │  │ 状态数据库│  │ 区块验证器   │ │
│  └──────────┘  └──────────┘  └──────────────┘ │
└────┬──────────────────────────────────────┬───┘
     │ P2P (DevP2P)                         │ JSON-RPC
     │ - eth/68                             │ - eth_*
     │ - snap/1                             │ - debug_*
     │                                      │ - txpool_*
┌────▼──────────────┐              ┌───────▼──────────┐
│  其他执行节点      │              │  外部客户端       │
│  (Peer Nodes)     │              │  (Wallets, DApps)│
└───────────────────┘              └──────────────────┘
```

---

## 2. 与共识客户端交互（Engine API）

### 用例 2.1: 接收新区块Payload

**参与者**: 共识客户端 → 执行客户端

**前置条件**:
- 共识客户端收到新的信标链区块
- 执行客户端处于同步状态

**主流程**:

1. **共识客户端调用** `engine_newPayloadV3`
   ```json
   {
     "method": "engine_newPayloadV3",
     "params": [
       {
         "parentHash": "0x...",
         "feeRecipient": "0x...",
         "stateRoot": "0x...",
         "receiptsRoot": "0x...",
         "logsBloom": "0x...",
         "prevRandao": "0x...",
         "blockNumber": "0x1234",
         "gasLimit": "0x1c9c380",
         "gasUsed": "0x12345",
         "timestamp": "0x6543210",
         "extraData": "0x",
         "baseFeePerGas": "0x7",
         "blockHash": "0x...",
         "transactions": ["0x...", "0x..."],
         "withdrawals": [...],
         "blobGasUsed": "0x20000",
         "excessBlobGas": "0x0"
       },
       ["0x..."], // expectedBlobVersionedHashes
       "0x..."    // parentBeaconBlockRoot
     ]
   }
   ```

2. **执行客户端验证**:
   - 验证父区块存在且已执行
   - 验证区块时间戳 > 父区块时间戳
   - 验证baseFeePerGas计算正确
   - 验证blobGasUsed和excessBlobGas

3. **执行交易**:
   ```rust
   // 领域层用例
   pub struct ExecutePayloadUseCase {
       state_db: Arc<dyn StateRepository>,
       tx_executor: Arc<dyn TransactionExecutor>,
       block_validator: Arc<dyn BlockValidator>,
   }

   impl ExecutePayloadUseCase {
       pub async fn execute(
           &self,
           payload: ExecutionPayload,
       ) -> Result<PayloadStatus, ExecutionError> {
           // 1. 验证区块头
           self.block_validator.validate_header(&payload.header)?;

           // 2. 创建状态快照
           let mut state = self.state_db.at_block(payload.parent_hash).await?;

           // 3. 执行交易
           let mut receipts = Vec::new();
           let mut gas_used = 0u64;

           for tx in payload.transactions {
               let receipt = self.tx_executor.execute(
                   &tx,
                   &mut state,
                   &payload.header,
               ).await?;

               gas_used += receipt.gas_used;
               receipts.push(receipt);
           }

           // 4. 处理提款（Withdrawals）
           for withdrawal in payload.withdrawals {
               state.add_balance(
                   withdrawal.address,
                   withdrawal.amount * 1_000_000_000, // Gwei to Wei
               )?;
           }

           // 5. 验证状态根
           let state_root = state.compute_root()?;
           if state_root != payload.state_root {
               return Ok(PayloadStatus::Invalid {
                   validation_error: "Invalid state root".into(),
               });
           }

           // 6. 持久化状态
           self.state_db.commit(state).await?;

           Ok(PayloadStatus::Valid {
               latest_valid_hash: payload.block_hash,
           })
       }
   }
   ```

4. **返回执行结果**:
   ```json
   {
     "status": "VALID",
     "latestValidHash": "0x...",
     "validationError": null
   }
   ```

**后置条件**:
- 新区块状态已持久化
- 区块标记为VALID/INVALID/SYNCING

---

### 用例 2.2: Forkchoice更新

**参与者**: 共识客户端 → 执行客户端

**前置条件**:
- 共识客户端确定新的链头

**主流程**:

1. **共识客户端调用** `engine_forkchoiceUpdatedV3`
   ```json
   {
     "method": "engine_forkchoiceUpdatedV3",
     "params": [
       {
         "headBlockHash": "0x...",
         "safeBlockHash": "0x...",
         "finalizedBlockHash": "0x..."
       },
       {
         "timestamp": "0x6543210",
         "prevRandao": "0x...",
         "suggestedFeeRecipient": "0x...",
         "withdrawals": [...],
         "parentBeaconBlockRoot": "0x..."
       }
     ]
   }
   ```

2. **执行客户端处理**:
   ```rust
   pub struct ForkchoiceUpdateUseCase {
       chain_state: Arc<dyn ChainStateRepository>,
       payload_builder: Arc<dyn PayloadBuilder>,
   }

   impl ForkchoiceUpdateUseCase {
       pub async fn execute(
           &self,
           forkchoice: ForkchoiceState,
           attrs: Option<PayloadAttributes>,
       ) -> Result<ForkchoiceUpdatedResponse, Error> {
           // 1. 更新链头
           self.chain_state.set_head(forkchoice.head_block_hash).await?;

           // 2. 标记安全区块
           self.chain_state.set_safe(forkchoice.safe_block_hash).await?;

           // 3. 标记最终确定区块
           self.chain_state.set_finalized(
               forkchoice.finalized_block_hash
           ).await?;

           // 4. 如果需要构建新payload
           let payload_id = if let Some(attrs) = attrs {
               let id = self.payload_builder.start_building(
                   forkchoice.head_block_hash,
                   attrs,
               ).await?;
               Some(id)
           } else {
               None
           };

           Ok(ForkchoiceUpdatedResponse {
               payload_status: PayloadStatus::Valid {
                   latest_valid_hash: forkchoice.head_block_hash,
               },
               payload_id,
           })
       }
   }
   ```

3. **后台构建Payload**:
   ```rust
   pub struct PayloadBuilderService {
       tx_pool: Arc<dyn TransactionPool>,
       state_db: Arc<dyn StateRepository>,
   }

   impl PayloadBuilderService {
       async fn build_payload(
           &self,
           parent: Hash,
           attrs: PayloadAttributes,
       ) -> Result<ExecutionPayload, Error> {
           // 1. 获取父状态
           let mut state = self.state_db.at_block(parent).await?;

           // 2. 从交易池选择交易
           let txs = self.tx_pool.select_transactions(
               attrs.gas_limit,
               |tx| tx.max_fee_per_gas >= attrs.base_fee
           ).await?;

           // 3. 执行交易并构建区块
           let (receipts, gas_used) = self.execute_transactions(
               &txs,
               &mut state,
               &attrs,
           ).await?;

           // 4. 计算状态根
           let state_root = state.compute_root()?;

           Ok(ExecutionPayload {
               parent_hash: parent,
               fee_recipient: attrs.suggested_fee_recipient,
               state_root,
               timestamp: attrs.timestamp,
               transactions: txs,
               // ... 其他字段
           })
       }
   }
   ```

**后置条件**:
- 链头、安全区块、最终确定区块已更新
- 如有需要，后台开始构建新payload

---

### 用例 2.3: 获取构建的Payload

**参与者**: 共识客户端 → 执行客户端

**主流程**:

1. **共识客户端调用** `engine_getPayloadV3`
   ```json
   {
     "method": "engine_getPayloadV3",
     "params": ["0x1234567890abcdef"]  // payloadId
   }
   ```

2. **执行客户端返回构建的payload**:
   ```rust
   pub struct GetPayloadUseCase {
       payload_store: Arc<dyn PayloadStore>,
   }

   impl GetPayloadUseCase {
       pub async fn execute(
           &self,
           payload_id: PayloadId,
       ) -> Result<GetPayloadResponse, Error> {
           let payload = self.payload_store
               .get_payload(payload_id)
               .await?
               .ok_or(Error::PayloadNotFound)?;

           Ok(GetPayloadResponse {
               execution_payload: payload.execution_payload,
               block_value: payload.block_value,
               blobs_bundle: payload.blobs_bundle,
               should_override_builder: false,
           })
       }
   }
   ```

---

## 3. P2P网络节点交互

### 用例 3.1: 区块传播（Block Propagation）

**参与者**: 执行节点 A ↔ 执行节点 B

**协议**: `eth/68`

**主流程**:

1. **节点A接收到新区块**（来自共识客户端）

2. **节点A广播NewBlock消息**:
   ```rust
   pub struct P2PBlockPropagationUseCase {
       peer_manager: Arc<dyn PeerManager>,
       block_store: Arc<dyn BlockRepository>,
   }

   impl P2PBlockPropagationUseCase {
       pub async fn propagate_block(
           &self,
           block: Block,
       ) -> Result<(), Error> {
           // 1. 获取活跃peers
           let peers = self.peer_manager.get_active_peers().await?;

           // 2. 广播到部分peers（sqrt(n)）
           let broadcast_count = (peers.len() as f64).sqrt() as usize;
           let selected_peers = peers.into_iter().take(broadcast_count);

           for peer in selected_peers {
               // 发送NewBlock消息 (0x07)
               peer.send_message(EthMessage::NewBlock {
                   block: block.clone(),
                   td: self.calculate_total_difficulty(&block).await?,
               }).await?;
           }

           // 3. 向其余peers发送NewBlockHashes (0x01)
           let remaining_peers = self.peer_manager
               .get_peers_without_block(block.hash())
               .await?;

           for peer in remaining_peers {
               peer.send_message(EthMessage::NewBlockHashes(vec![
                   BlockHashNumber {
                       hash: block.hash(),
                       number: block.number,
                   }
               ])).await?;
           }

           Ok(())
       }
   }
   ```

3. **节点B接收NewBlockHashes**:
   ```rust
   pub struct HandleNewBlockHashesUseCase {
       block_store: Arc<dyn BlockRepository>,
       sync_service: Arc<dyn SyncService>,
   }

   impl HandleNewBlockHashesUseCase {
       pub async fn handle(
           &self,
           peer: PeerId,
           hashes: Vec<BlockHashNumber>,
       ) -> Result<(), Error> {
           let mut unknown_hashes = Vec::new();

           // 检查哪些区块我们没有
           for hash_num in hashes {
               if !self.block_store.has_block(hash_num.hash).await? {
                   unknown_hashes.push(hash_num.hash);
               }
           }

           // 请求缺失的区块
           if !unknown_hashes.is_empty() {
               self.sync_service.request_blocks(
                   peer,
                   unknown_hashes,
               ).await?;
           }

           Ok(())
       }
   }
   ```

---

### 用例 3.2: 交易传播（Transaction Propagation）

**协议**: `eth/68` - Transactions消息

**主流程**:

1. **节点A接收新交易**（via JSON-RPC）

2. **验证并添加到交易池**:
   ```rust
   pub struct AddTransactionUseCase {
       tx_pool: Arc<dyn TransactionPool>,
       tx_validator: Arc<dyn TransactionValidator>,
       p2p_service: Arc<dyn P2PService>,
   }

   impl AddTransactionUseCase {
       pub async fn execute(
           &self,
           tx: Transaction,
       ) -> Result<TxHash, Error> {
           // 1. 验证交易
           self.tx_validator.validate(&tx)?;

           // 2. 添加到交易池
           let tx_hash = self.tx_pool.add(tx.clone()).await?;

           // 3. 广播到peers
           self.p2p_service.broadcast_transactions(vec![tx]).await?;

           Ok(tx_hash)
       }
   }
   ```

3. **P2P广播实现**:
   ```rust
   pub struct P2PTransactionPropagation {
       peer_manager: Arc<dyn PeerManager>,
   }

   impl P2PTransactionPropagation {
       pub async fn broadcast_transactions(
           &self,
           txs: Vec<Transaction>,
       ) -> Result<(), Error> {
           let peers = self.peer_manager.get_active_peers().await?;

           // 随机选择peers避免重复传播
           let selected_peers = self.select_random_peers(&peers, 0.5);

           for peer in selected_peers {
               // 过滤peer已知的交易
               let unknown_txs = txs.iter()
                   .filter(|tx| !peer.has_transaction(tx.hash()))
                   .cloned()
                   .collect::<Vec<_>>();

               if !unknown_txs.is_empty() {
                   peer.send_message(EthMessage::Transactions(unknown_txs))
                       .await?;
               }
           }

           Ok(())
       }
   }
   ```

---

### 用例 3.3: 状态同步（Snap Sync）

**协议**: `snap/1`

**场景**: 新节点快速同步到最新状态

**主流程**:

1. **请求账户范围**:
   ```rust
   pub struct SnapSyncUseCase {
       peer_manager: Arc<dyn PeerManager>,
       state_db: Arc<dyn StateRepository>,
   }

   impl SnapSyncUseCase {
       pub async fn sync_accounts(
           &self,
           root_hash: Hash,
           start_hash: Hash,
           limit: u64,
       ) -> Result<(), Error> {
           // 1. 选择支持snap协议的peer
           let peer = self.peer_manager
               .get_peer_with_capability("snap", 1)
               .await?;

           // 2. 请求账户范围
           let response = peer.request(SnapMessage::GetAccountRange {
               id: rand::random(),
               root_hash,
               starting_hash: start_hash,
               limit_hash: Hash::MAX,
               response_bytes: limit,
           }).await?;

           // 3. 验证并存储账户
           match response {
               SnapMessage::AccountRange { accounts, proof, .. } => {
                   // 验证Merkle证明
                   self.verify_account_range_proof(
                       root_hash,
                       &accounts,
                       &proof,
                   )?;

                   // 存储账户
                   for account in accounts {
                       self.state_db.insert_account(account).await?;
                   }
               }
               _ => return Err(Error::UnexpectedMessage),
           }

           Ok(())
       }
   }
   ```

2. **请求存储范围**:
   ```rust
   impl SnapSyncUseCase {
       pub async fn sync_storage(
           &self,
           account: Address,
           storage_root: Hash,
       ) -> Result<(), Error> {
           let peer = self.peer_manager
               .get_peer_with_capability("snap", 1)
               .await?;

           let response = peer.request(SnapMessage::GetStorageRanges {
               id: rand::random(),
               root_hash: storage_root,
               account_hashes: vec![account.hash()],
               starting_hash: Hash::ZERO,
               limit_hash: Hash::MAX,
               response_bytes: 500_000,
           }).await?;

           match response {
               SnapMessage::StorageRanges { slots, proof, .. } => {
                   self.verify_storage_proof(&slots, &proof)?;
                   self.state_db.insert_storage(account, slots).await?;
               }
               _ => return Err(Error::UnexpectedMessage),
           }

           Ok(())
       }
   }
   ```

---

## 4. JSON-RPC客户端交互

### 用例 4.1: 提交交易（eth_sendRawTransaction）

**参与者**: 外部客户端（钱包）→ 执行客户端

**主流程**:

```rust
pub struct SubmitRawTransactionUseCase {
    tx_pool: Arc<dyn TransactionPool>,
    tx_decoder: Arc<dyn TransactionDecoder>,
    p2p_service: Arc<dyn P2PService>,
}

impl SubmitRawTransactionUseCase {
    pub async fn execute(
        &self,
        raw_tx: Bytes,
    ) -> Result<TxHash, RpcError> {
        // 1. 解码交易
        let tx = self.tx_decoder.decode(raw_tx)?;

        // 2. 基本验证
        self.validate_transaction(&tx)?;

        // 3. 恢复发送者地址
        let sender = tx.recover_sender()
            .ok_or(RpcError::InvalidSignature)?;

        // 4. 检查nonce
        let account_nonce = self.tx_pool
            .get_account_nonce(sender)
            .await?;

        if tx.nonce < account_nonce {
            return Err(RpcError::NonceTooLow);
        }

        // 5. 添加到交易池
        let tx_hash = self.tx_pool.add(tx.clone()).await?;

        // 6. 广播到P2P网络
        self.p2p_service.broadcast_transactions(vec![tx]).await?;

        Ok(tx_hash)
    }

    fn validate_transaction(&self, tx: &Transaction) -> Result<(), RpcError> {
        // 检查gas limit
        if tx.gas_limit > 30_000_000 {
            return Err(RpcError::GasLimitTooHigh);
        }

        // 检查gas price
        if tx.max_fee_per_gas < 1_000_000_000 {  // 1 Gwei minimum
            return Err(RpcError::GasPriceTooLow);
        }

        Ok(())
    }
}
```

---

### 用例 4.2: 查询账户余额（eth_getBalance）

**参与者**: 外部客户端 → 执行客户端

**主流程**:

```rust
pub struct GetBalanceUseCase {
    state_db: Arc<dyn StateRepository>,
    block_resolver: Arc<dyn BlockResolver>,
}

impl GetBalanceUseCase {
    pub async fn execute(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<U256, RpcError> {
        // 1. 解析区块标识符
        let block_hash = self.block_resolver
            .resolve_block_id(block_id)
            .await?;

        // 2. 获取该区块的状态
        let state = self.state_db.at_block(block_hash).await?;

        // 3. 查询账户余额
        let account = state.get_account(address).await?;

        Ok(account.map(|acc| acc.balance).unwrap_or(U256::ZERO))
    }
}

pub trait BlockResolver: Send + Sync {
    async fn resolve_block_id(&self, id: BlockId) -> Result<Hash, Error>;
}

impl BlockResolver for BlockResolverService {
    async fn resolve_block_id(&self, id: BlockId) -> Result<Hash, Error> {
        match id {
            BlockId::Hash(hash) => Ok(hash),
            BlockId::Number(num) => {
                self.get_block_hash_by_number(num).await
            }
            BlockId::Latest => self.get_latest_block_hash().await,
            BlockId::Earliest => Ok(GENESIS_HASH),
            BlockId::Pending => self.get_pending_block_hash().await,
            BlockId::Finalized => self.get_finalized_block_hash().await,
            BlockId::Safe => self.get_safe_block_hash().await,
        }
    }
}
```

---

### 用例 4.3: 执行调用（eth_call）

**参与者**: 外部客户端 → 执行客户端

**用途**: 模拟交易执行，不改变状态

**主流程**:

```rust
pub struct CallTransactionUseCase {
    state_db: Arc<dyn StateRepository>,
    evm: Arc<dyn EvmExecutor>,
    block_resolver: Arc<dyn BlockResolver>,
}

impl CallTransactionUseCase {
    pub async fn execute(
        &self,
        call: CallRequest,
        block_id: BlockId,
    ) -> Result<Bytes, RpcError> {
        // 1. 解析区块
        let block_hash = self.block_resolver
            .resolve_block_id(block_id)
            .await?;

        let block = self.state_db
            .get_block(block_hash)
            .await?
            .ok_or(RpcError::BlockNotFound)?;

        // 2. 获取状态（只读）
        let state = self.state_db.at_block(block_hash).await?;

        // 3. 构建EVM环境
        let env = EvmEnv {
            block_number: block.number,
            timestamp: block.timestamp,
            gas_limit: call.gas.unwrap_or(block.gas_limit),
            base_fee: block.base_fee_per_gas,
            coinbase: block.fee_recipient,
            difficulty: U256::ZERO,  // Post-merge always 0
            prevrandao: block.prev_randao,
        };

        // 4. 执行调用
        let result = self.evm.call(
            call.from.unwrap_or_default(),
            call.to,
            call.data.unwrap_or_default(),
            call.value.unwrap_or_default(),
            call.gas.unwrap_or(u64::MAX),
            state,
            env,
        ).await?;

        match result {
            ExecutionResult::Success { output, .. } => Ok(output),
            ExecutionResult::Revert { output, .. } => {
                Err(RpcError::Revert(output))
            }
            ExecutionResult::Halt { reason, .. } => {
                Err(RpcError::EvmHalt(reason))
            }
        }
    }
}
```

---

### 用例 4.4: 估算Gas（eth_estimateGas）

**主流程**:

```rust
pub struct EstimateGasUseCase {
    call_usecase: Arc<CallTransactionUseCase>,
}

impl EstimateGasUseCase {
    pub async fn execute(
        &self,
        mut call: CallRequest,
        block_id: BlockId,
    ) -> Result<u64, RpcError> {
        // 二分搜索找到最小gas
        let mut low = 21_000u64;  // 最小交易gas
        let mut high = call.gas.unwrap_or(30_000_000);

        // 先测试上限是否足够
        call.gas = Some(high);
        if self.call_usecase.execute(call.clone(), block_id).await.is_err() {
            return Err(RpcError::ExecutionError);
        }

        // 二分搜索
        while low < high {
            let mid = (low + high) / 2;
            call.gas = Some(mid);

            match self.call_usecase.execute(call.clone(), block_id).await {
                Ok(_) => high = mid,
                Err(_) => low = mid + 1,
            }
        }

        // 添加5%余量
        Ok(low * 105 / 100)
    }
}
```

---

## 5. 内部组件交互

### 用例 5.1: 交易池管理

**组件**: TransactionPool

**职责**:
- 维护待处理交易队列
- 按gas价格排序
- 淘汰过期/低价交易
- 提供交易给区块构建器

**实现**:

```rust
pub struct TransactionPoolService {
    pending: Arc<RwLock<BTreeMap<Address, Vec<Transaction>>>>,
    queued: Arc<RwLock<BTreeMap<Address, Vec<Transaction>>>>,
    state_db: Arc<dyn StateRepository>,
    config: TxPoolConfig,
}

impl TransactionPool for TransactionPoolService {
    async fn add(&self, tx: Transaction) -> Result<TxHash, Error> {
        let sender = tx.recover_sender()
            .ok_or(Error::InvalidSignature)?;

        // 获取账户当前nonce
        let account_nonce = self.get_account_nonce(sender).await?;

        let mut pending = self.pending.write().await;
        let mut queued = self.queued.write().await;

        // 检查是否可立即执行
        if tx.nonce == account_nonce {
            // 添加到pending队列
            pending.entry(sender)
                .or_insert_with(Vec::new)
                .push(tx.clone());

            // 尝试提升queued中的后续交易
            self.promote_transactions(sender, &mut pending, &mut queued).await?;
        } else if tx.nonce > account_nonce {
            // 添加到queued队列
            queued.entry(sender)
                .or_insert_with(Vec::new)
                .push(tx.clone());
        } else {
            return Err(Error::NonceTooLow);
        }

        // 检查交易池限制
        self.enforce_limits(&mut pending, &mut queued).await?;

        Ok(tx.hash())
    }

    async fn select_transactions(
        &self,
        gas_limit: u64,
        filter: impl Fn(&Transaction) -> bool,
    ) -> Result<Vec<Transaction>, Error> {
        let pending = self.pending.read().await;
        let mut selected = Vec::new();
        let mut total_gas = 0u64;

        // 按gas价格排序（从高到低）
        let mut sorted_txs: Vec<_> = pending.values()
            .flatten()
            .filter(|tx| filter(tx))
            .collect();

        sorted_txs.sort_by(|a, b| {
            b.max_fee_per_gas.cmp(&a.max_fee_per_gas)
        });

        // 选择交易直到达到gas limit
        for tx in sorted_txs {
            if total_gas + tx.gas_limit > gas_limit {
                break;
            }
            total_gas += tx.gas_limit;
            selected.push(tx.clone());
        }

        Ok(selected)
    }
}
```

---

### 用例 5.2: 状态数据库管理

**组件**: StateDatabase

**职责**:
- 维护账户状态树（Merkle Patricia Trie）
- 支持历史状态查询
- 状态快照和回滚

**实现**:

```rust
pub struct StateDatabase {
    db: Arc<dyn KeyValueStore>,
    cache: Arc<RwLock<LruCache<Hash, Account>>>,
}

impl StateRepository for StateDatabase {
    async fn at_block(&self, block_hash: Hash) -> Result<State, Error> {
        // 获取区块对应的状态根
        let block = self.get_block(block_hash).await?
            .ok_or(Error::BlockNotFound)?;

        Ok(State {
            root: block.state_root,
            db: self.db.clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            changed: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    async fn commit(&self, state: State) -> Result<Hash, Error> {
        let changed = state.changed.read().await;

        // 批量写入变更
        let mut batch = self.db.batch_write();

        for (address, account) in changed.iter() {
            let key = keccak256(address.as_bytes());
            let value = rlp::encode(account);
            batch.put(key.as_slice(), &value);
        }

        batch.commit().await?;

        // 计算新的状态根
        let new_root = self.compute_state_root(&changed).await?;

        Ok(new_root)
    }
}

pub struct State {
    root: Hash,
    db: Arc<dyn KeyValueStore>,
    cache: Arc<RwLock<HashMap<Address, Account>>>,
    changed: Arc<RwLock<HashMap<Address, Account>>>,
}

impl State {
    pub async fn get_account(&self, address: Address) -> Result<Option<Account>, Error> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(account) = cache.get(&address) {
                return Ok(Some(account.clone()));
            }
        }

        // 从MPT读取
        let key = keccak256(address.as_bytes());
        let value = self.db.get(key.as_slice()).await?;

        let account = value.map(|v| rlp::decode(&v)).transpose()?;

        // 更新缓存
        if let Some(ref acc) = account {
            let mut cache = self.cache.write().await;
            cache.insert(address, acc.clone());
        }

        Ok(account)
    }

    pub async fn set_balance(
        &mut self,
        address: Address,
        balance: U256,
    ) -> Result<(), Error> {
        let mut account = self.get_account(address)
            .await?
            .unwrap_or_default();

        account.balance = balance;

        let mut changed = self.changed.write().await;
        changed.insert(address, account);

        Ok(())
    }

    pub fn compute_root(&self) -> Result<Hash, Error> {
        // 使用Merkle Patricia Trie计算状态根
        // 实现省略...
        todo!()
    }
}
```

---

### 用例 5.3: 区块验证

**组件**: BlockValidator

**职责**:
- 验证区块头
- 验证交易merkle root
- 验证状态转换

**实现**:

```rust
pub struct BlockValidatorService {
    consensus_params: ConsensusParams,
}

impl BlockValidator for BlockValidatorService {
    fn validate_header(&self, header: &Header) -> Result<(), ValidationError> {
        // 1. 验证extraData长度
        if header.extra_data.len() > 32 {
            return Err(ValidationError::ExtraDataTooLong);
        }

        // 2. 验证时间戳
        if header.timestamp <= header.parent_timestamp {
            return Err(ValidationError::InvalidTimestamp);
        }

        // 3. 验证baseFeePerGas（EIP-1559）
        let expected_base_fee = self.calculate_base_fee(
            header.parent_gas_used,
            header.parent_gas_limit,
            header.parent_base_fee,
        );

        if header.base_fee_per_gas != expected_base_fee {
            return Err(ValidationError::InvalidBaseFee);
        }

        // 4. 验证blobGasUsed和excessBlobGas（EIP-4844）
        if let Some(blob_gas_used) = header.blob_gas_used {
            if blob_gas_used % self.consensus_params.blob_gas_per_blob != 0 {
                return Err(ValidationError::InvalidBlobGasUsed);
            }

            let expected_excess = self.calculate_excess_blob_gas(
                header.parent_excess_blob_gas.unwrap_or(0),
                header.parent_blob_gas_used.unwrap_or(0),
            );

            if header.excess_blob_gas != Some(expected_excess) {
                return Err(ValidationError::InvalidExcessBlobGas);
            }
        }

        Ok(())
    }

    fn validate_block(&self, block: &Block) -> Result<(), ValidationError> {
        // 1. 验证区块头
        self.validate_header(&block.header)?;

        // 2. 验证交易merkle root
        let txs_root = calculate_transactions_root(&block.transactions);
        if txs_root != block.header.transactions_root {
            return Err(ValidationError::InvalidTransactionsRoot);
        }

        // 3. 验证withdrawals root
        if let Some(withdrawals) = &block.withdrawals {
            let withdrawals_root = calculate_withdrawals_root(withdrawals);
            if Some(withdrawals_root) != block.header.withdrawals_root {
                return Err(ValidationError::InvalidWithdrawalsRoot);
            }
        }

        // 4. 验证gas使用
        let total_gas: u64 = block.transactions
            .iter()
            .map(|tx| tx.gas_limit)
            .sum();

        if total_gas > block.header.gas_limit {
            return Err(ValidationError::BlockGasLimitExceeded);
        }

        Ok(())
    }

    fn calculate_base_fee(
        &self,
        parent_gas_used: u64,
        parent_gas_limit: u64,
        parent_base_fee: u64,
    ) -> u64 {
        // EIP-1559 base fee计算
        let parent_gas_target = parent_gas_limit / 2;

        if parent_gas_used == parent_gas_target {
            return parent_base_fee;
        }

        if parent_gas_used > parent_gas_target {
            let gas_used_delta = parent_gas_used - parent_gas_target;
            let base_fee_delta = std::cmp::max(
                parent_base_fee * gas_used_delta / parent_gas_target / 8,
                1,
            );
            parent_base_fee + base_fee_delta
        } else {
            let gas_used_delta = parent_gas_target - parent_gas_used;
            let base_fee_delta =
                parent_base_fee * gas_used_delta / parent_gas_target / 8;
            parent_base_fee.saturating_sub(base_fee_delta)
        }
    }
}
```

---

## 6. 用例实现架构

### 6.1 Clean Architecture分层

```
┌─────────────────────────────────────────────────┐
│              Interface Layer                    │
│  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ Engine API   │  │ JSON-RPC                 │ │
│  │ Controllers  │  │ Controllers              │ │
│  └──────┬───────┘  └─────────┬────────────────┘ │
└─────────┼──────────────────────┼──────────────────┘
          │                      │
┌─────────▼──────────────────────▼──────────────────┐
│             Application Layer (Use Cases)         │
│  ┌──────────────────┐  ┌────────────────────────┐ │
│  │ ExecutePayload   │  │ SubmitTransaction      │ │
│  │ UseCase          │  │ UseCase                │ │
│  └────────┬─────────┘  └──────────┬─────────────┘ │
└───────────┼────────────────────────┼───────────────┘
            │                        │
┌───────────▼────────────────────────▼───────────────┐
│              Domain Layer                          │
│  ┌──────────┐  ┌──────────┐  ┌─────────────────┐  │
│  │ Block    │  │ Transaction│  │ State          │  │
│  │ Entity   │  │ Entity     │  │ Entity         │  │
│  └──────────┘  └──────────┘  └─────────────────┘  │
│                                                    │
│  ┌──────────────────────────────────────────────┐ │
│  │ Repository Interfaces (Ports)                │ │
│  │ - StateRepository                            │ │
│  │ - BlockRepository                            │ │
│  │ - TransactionPool                            │ │
│  └──────────────────────────────────────────────┘ │
└────────────────────────┬───────────────────────────┘
                         │
┌────────────────────────▼───────────────────────────┐
│           Infrastructure Layer                     │
│  ┌──────────────────┐  ┌───────────────────────┐  │
│  │ PostgresState    │  │ MPTStateDatabase      │  │
│  │ Repository       │  │                       │  │
│  └──────────────────┘  └───────────────────────┘  │
│  ┌──────────────────┐  ┌───────────────────────┐  │
│  │ P2PService       │  │ EvmExecutor           │  │
│  └──────────────────┘  └───────────────────────┘  │
└────────────────────────────────────────────────────┘
```

### 6.2 依赖注入示例

```rust
// main.rs - 应用入口
#[tokio::main]
async fn main() {
    // === Infrastructure Layer ===
    let db = Arc::new(RocksDB::open("./data").unwrap());

    let state_db: Arc<dyn StateRepository> = Arc::new(
        MPTStateDatabase::new(db.clone())
    );

    let block_repo: Arc<dyn BlockRepository> = Arc::new(
        BlockRepositoryImpl::new(db.clone())
    );

    let tx_pool: Arc<dyn TransactionPool> = Arc::new(
        TransactionPoolService::new(state_db.clone())
    );

    let evm: Arc<dyn EvmExecutor> = Arc::new(
        ReVmExecutor::new()
    );

    let p2p: Arc<dyn P2PService> = Arc::new(
        P2PServiceImpl::new(config.p2p)
    );

    // === Use Cases ===
    let execute_payload = Arc::new(ExecutePayloadUseCase {
        state_db: state_db.clone(),
        tx_executor: evm.clone(),
        block_validator: Arc::new(BlockValidatorService::new()),
    });

    let forkchoice_update = Arc::new(ForkchoiceUpdateUseCase {
        chain_state: block_repo.clone(),
        payload_builder: Arc::new(PayloadBuilderService {
            tx_pool: tx_pool.clone(),
            state_db: state_db.clone(),
        }),
    });

    let submit_tx = Arc::new(SubmitRawTransactionUseCase {
        tx_pool: tx_pool.clone(),
        tx_decoder: Arc::new(RlpTransactionDecoder),
        p2p_service: p2p.clone(),
    });

    // === Interface Layer ===
    // Engine API
    let engine_api = Arc::new(EngineApiController {
        execute_payload: execute_payload.clone(),
        forkchoice_update: forkchoice_update.clone(),
    });

    // JSON-RPC
    let rpc_api = Arc::new(JsonRpcController {
        submit_tx: submit_tx.clone(),
        get_balance: Arc::new(GetBalanceUseCase {
            state_db: state_db.clone(),
            block_resolver: block_repo.clone(),
        }),
    });

    // 启动服务
    tokio::spawn(start_engine_api(engine_api));
    tokio::spawn(start_json_rpc(rpc_api));
    tokio::spawn(start_p2p(p2p));

    // 等待信号
    tokio::signal::ctrl_c().await.unwrap();
}
```

### 6.3 测试策略

**领域层测试**（无外部依赖）:
```rust
#[cfg(test)]
mod domain_tests {
    #[test]
    fn test_block_validation() {
        let validator = BlockValidatorService::new();

        let header = Header {
            base_fee_per_gas: 1000,
            parent_base_fee: 1000,
            parent_gas_used: 15_000_000,
            parent_gas_limit: 30_000_000,
            // ...
        };

        assert!(validator.validate_header(&header).is_ok());
    }
}
```

**应用层测试**（Mock依赖）:
```rust
#[cfg(test)]
mod usecase_tests {
    use mockall::mock;

    mock! {
        StateRepo {}
        #[async_trait]
        impl StateRepository for StateRepo {
            async fn at_block(&self, hash: Hash) -> Result<State, Error>;
            async fn commit(&self, state: State) -> Result<Hash, Error>;
        }
    }

    #[tokio::test]
    async fn test_execute_payload() {
        let mut mock_state = MockStateRepo::new();
        mock_state.expect_at_block()
            .returning(|_| Ok(State::default()));

        let usecase = ExecutePayloadUseCase {
            state_db: Arc::new(mock_state),
            tx_executor: Arc::new(MockEvmExecutor::new()),
            block_validator: Arc::new(BlockValidatorService::new()),
        };

        let payload = create_test_payload();
        let result = usecase.execute(payload).await;

        assert!(result.is_ok());
    }
}
```

---

## 总结

本文档详细描述了执行客户端在以太坊网络中的核心交互用例：

1. **与共识客户端交互**: Engine API实现（newPayload, forkchoiceUpdated, getPayload）
2. **P2P网络交互**: 区块传播、交易传播、状态同步（snap sync）
3. **JSON-RPC交互**: 交易提交、状态查询、调用模拟
4. **内部组件**: 交易池管理、状态数据库、区块验证

所有用例遵循Clean Architecture原则：
- **领域层**: 纯业务逻辑，无外部依赖
- **应用层**: 用例编排，依赖抽象接口
- **接口层**: 协议适配和数据转换
- **基础设施层**: 具体技术实现

这确保了代码的可测试性、可维护性和领域逻辑的独立性。
