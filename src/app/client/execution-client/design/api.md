# 执行客户端 JSON-RPC API 规范

基于以太坊官方标准的执行客户端 JSON-RPC API 接口设计文档。

## 目录
- [1. 概述](#1-概述)
- [2. 标准以太坊 JSON-RPC API](#2-标准以太坊-json-rpc-api)
- [3. Engine API (共识层通信)](#3-engine-api-共识层通信)
- [4. Admin & Debug API](#4-admin--debug-api)
- [5. Clean Architecture 实现](#5-clean-architecture-实现)
- [6. 性能优化指南](#6-性能优化指南)
- [7. 错误处理规范](#7-错误处理规范)

---

## 1. 概述

### 1.1 JSON-RPC 协议

JSON-RPC 是一个无状态、轻量级的远程过程调用（RPC）协议。

**请求格式**:
```json
{
  "jsonrpc": "2.0",
  "method": "eth_blockNumber",
  "params": [],
  "id": 1
}
```

**响应格式**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x1234567"
}
```

**错误响应**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "Error message",
    "data": "Additional error data"
  }
}
```

### 1.2 数据类型约定

| 类型 | 描述 | 示例 |
|------|------|------|
| `QUANTITY` | 十六进制编码的整数 | `"0x1234"` |
| `DATA` | 十六进制编码的字节数组 | `"0xabcd..."` |
| `TAG` | 特殊区块标识符 | `"latest"`, `"earliest"`, `"pending"`, `"safe"`, `"finalized"` |
| `Boolean` | 布尔值 | `true`, `false` |

### 1.3 区块标识符

- `"earliest"`: 创世区块
- `"latest"`: 最新区块（链头）
- `"pending"`: 待处理区块（交易池中的交易）
- `"safe"`: 安全区块（可能被重组的最后一个区块）
- `"finalized"`: 最终确定区块（不可逆）
- `QUANTITY`: 特定区块号（如 `"0x1234"`）
- Block hash object: `{"blockHash": "0x...", "requireCanonical": true}`

---

## 2. 标准以太坊 JSON-RPC API

### 2.1 网络信息

#### web3_clientVersion

返回当前客户端版本。

**参数**: 无

**返回值**: `String` - 客户端版本字符串

**示例**:
```json
// 请求
{"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":"Reth/v1.0.0/linux-x86_64/rustc1.75.0"}
```

**实现**:
```rust
// interfaces/rpc/handlers/web3.rs
pub struct Web3ApiHandler {
    client_version: String,
}

impl Web3ApiHandler {
    pub fn client_version(&self) -> RpcResult<String> {
        Ok(self.client_version.clone())
    }
}
```

---

#### net_version

返回当前网络 ID。

**参数**: 无

**返回值**: `String` - 网络 ID
- `"1"`: Mainnet
- `"11155111"`: Sepolia
- `"17000"`: Holesky

**示例**:
```json
// 请求
{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":"1"}
```

---

#### net_listening

返回客户端是否正在监听网络连接。

**参数**: 无

**返回值**: `Boolean` - 如果正在监听返回 `true`

**示例**:
```json
{"jsonrpc":"2.0","id":1,"result":true}
```

---

#### net_peerCount

返回当前连接的对等节点数量。

**参数**: 无

**返回值**: `QUANTITY` - 对等节点数量（十六进制）

**示例**:
```json
{"jsonrpc":"2.0","id":1,"result":"0x32"}  // 50 peers
```

**实现**:
```rust
pub struct NetApiHandler {
    peer_manager: Arc<dyn PeerManager>,
}

impl NetApiHandler {
    pub async fn peer_count(&self) -> RpcResult<U256> {
        let count = self.peer_manager.active_peer_count().await?;
        Ok(U256::from(count))
    }
}
```

---

### 2.2 账户相关

#### eth_accounts

返回客户端拥有的地址列表。

**参数**: 无

**返回值**: `Array<DATA>` - 20 字节的地址数组

**示例**:
```json
{
  "jsonrpc":"2.0",
  "id":1,
  "result":["0x407d73d8a49eeb85d32cf465507dd71d507100c1"]
}
```

**注意**: 现代客户端通常返回空数组，钱包管理由外部工具处理。

---

#### eth_getBalance

返回指定地址在指定区块的余额。

**参数**:
1. `DATA`, 20 Bytes - 要查询的地址
2. `QUANTITY|TAG` - 区块号或区块标签

**返回值**: `QUANTITY` - 当前余额（Wei 单位）

**示例**:
```json
// 请求
{
  "jsonrpc":"2.0",
  "method":"eth_getBalance",
  "params":["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"],
  "id":1
}

// 响应
{
  "jsonrpc":"2.0",
  "id":1,
  "result":"0x0234c8a3397aab58"  // 158972490234375000
}
```

**实现**:
```rust
// domain/usecases/get_balance.rs
pub struct GetBalanceUseCase {
    state_db: Arc<dyn StateRepository>,
    block_resolver: Arc<dyn BlockResolver>,
}

impl GetBalanceUseCase {
    pub async fn execute(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<U256, UseCaseError> {
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

// interfaces/rpc/handlers/eth.rs
pub struct EthApiHandler {
    get_balance: Arc<GetBalanceUseCase>,
}

impl EthApiHandler {
    pub async fn get_balance(
        &self,
        address: Address,
        block: BlockNumberOrTag,
    ) -> RpcResult<U256> {
        let block_id = self.convert_block_id(block)?;

        self.get_balance
            .execute(address, block_id)
            .await
            .map_err(|e| RpcError::internal_error(e.to_string()))
    }
}
```

---

#### eth_getStorageAt

返回指定地址存储位置的值。

**参数**:
1. `DATA`, 20 Bytes - 存储地址
2. `QUANTITY` - 存储位置
3. `QUANTITY|TAG` - 区块号或标签

**返回值**: `DATA` - 该存储位置的值

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getStorageAt",
  "params":[
    "0x295a70b2de5e3953354a6a8344e616ed314d7251",
    "0x0",
    "latest"
  ],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x0000000000000000000000000000000000000000000000000000000000000000"}
```

**实现**:
```rust
pub struct GetStorageAtUseCase {
    state_db: Arc<dyn StateRepository>,
    block_resolver: Arc<dyn BlockResolver>,
}

impl GetStorageAtUseCase {
    pub async fn execute(
        &self,
        address: Address,
        position: U256,
        block_id: BlockId,
    ) -> Result<H256, UseCaseError> {
        let block_hash = self.block_resolver.resolve_block_id(block_id).await?;
        let state = self.state_db.at_block(block_hash).await?;

        let value = state.get_storage(address, position).await?;
        Ok(H256::from(value))
    }
}
```

---

#### eth_getTransactionCount

返回从指定地址发送的交易数量（nonce）。

**参数**:
1. `DATA`, 20 Bytes - 地址
2. `QUANTITY|TAG` - 区块号或标签

**返回值**: `QUANTITY` - 从该地址发送的交易数量

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getTransactionCount",
  "params":["0x407d73d8a49eeb85d32cf465507dd71d507100c1", "latest"],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x1"}
```

**实现**:
```rust
pub struct GetTransactionCountUseCase {
    state_db: Arc<dyn StateRepository>,
    block_resolver: Arc<dyn BlockResolver>,
    tx_pool: Arc<dyn TransactionPool>,
}

impl GetTransactionCountUseCase {
    pub async fn execute(
        &self,
        address: Address,
        block_id: BlockId,
    ) -> Result<u64, UseCaseError> {
        match block_id {
            BlockId::Pending => {
                // 包括交易池中的待处理交易
                let nonce = self.tx_pool.get_account_nonce(address).await?;
                Ok(nonce)
            }
            _ => {
                let block_hash = self.block_resolver.resolve_block_id(block_id).await?;
                let state = self.state_db.at_block(block_hash).await?;
                let account = state.get_account(address).await?;
                Ok(account.map(|acc| acc.nonce).unwrap_or(0))
            }
        }
    }
}
```

---

#### eth_getCode

返回指定地址的代码（智能合约字节码）。

**参数**:
1. `DATA`, 20 Bytes - 地址
2. `QUANTITY|TAG` - 区块号或标签

**返回值**: `DATA` - 该地址的代码

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getCode",
  "params":["0xa94f5374fce5edbc8e2a8697c15331677e6ebf0b", "latest"],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x600160008035811a818181146012578301005b601b6001356025565b8060005260206000f25b600060078202905091905056"}
```

---

### 2.3 区块相关

#### eth_blockNumber

返回最新区块号。

**参数**: 无

**返回值**: `QUANTITY` - 最新区块号

**示例**:
```json
{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x1234567"}
```

**性能优化**:
```rust
pub struct EthBlockNumberHandler {
    // 缓存最新区块号，避免频繁数据库查询
    cached_block_number: Arc<AtomicU64>,
    chain_head_subscriber: Arc<dyn ChainHeadSubscriber>,
}

impl EthBlockNumberHandler {
    pub fn new(subscriber: Arc<dyn ChainHeadSubscriber>) -> Self {
        let handler = Self {
            cached_block_number: Arc::new(AtomicU64::new(0)),
            chain_head_subscriber: subscriber,
        };

        // 订阅链头更新
        let cached = handler.cached_block_number.clone();
        tokio::spawn(async move {
            let mut stream = subscriber.subscribe().await;
            while let Some(head) = stream.next().await {
                cached.store(head.number, Ordering::Release);
            }
        });

        handler
    }

    pub fn block_number(&self) -> RpcResult<U64> {
        let number = self.cached_block_number.load(Ordering::Acquire);
        Ok(U64::from(number))
    }
}
```

---

#### eth_getBlockByHash

根据区块哈希返回区块信息。

**参数**:
1. `DATA`, 32 Bytes - 区块哈希
2. `Boolean` - 如果为 `true`，返回完整交易对象；如果为 `false`，仅返回交易哈希

**返回值**: `Object` - 区块对象，如果区块不存在返回 `null`

**区块对象结构**:
```json
{
  "number": "0x1b4",           // 区块号
  "hash": "0xe670ec64341771606e55d6b4ca35a1a6b75ee3d5145a99d05921026d1527331",
  "parentHash": "0x9646252be9520f6e71339a8df9c55e4d7619deeb018d2a3f2d21fc165dde5eb5",
  "nonce": "0x0000000000000000",
  "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
  "logsBloom": "0x...",
  "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
  "stateRoot": "0xd5855eb08b3387c0af375e9cdb6acfc05eb8f519e419b874b6ff2ffda7ed1dff",
  "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
  "miner": "0x4e65fda2159562a496f9f3522f89122a3088497a",
  "difficulty": "0x0",          // Post-merge 总是 0
  "totalDifficulty": "0xc70d815d562d3cfa955",
  "extraData": "0x",
  "size": "0x027f07",
  "gasLimit": "0x1c9c380",
  "gasUsed": "0x12345",
  "timestamp": "0x54e34e8e",
  "transactions": [...],        // 交易数组或哈希数组
  "uncles": [],                 // Post-merge 总是空
  "baseFeePerGas": "0x7",       // EIP-1559
  "withdrawals": [...],         // Post-Shanghai
  "withdrawalsRoot": "0x...",   // Post-Shanghai
  "blobGasUsed": "0x20000",     // EIP-4844
  "excessBlobGas": "0x0",       // EIP-4844
  "parentBeaconBlockRoot": "0x..."  // EIP-4788
}
```

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getBlockByHash",
  "params":[
    "0xe670ec64341771606e55d6b4ca35a1a6b75ee3d5145a99d05921026d1527331",
    true
  ],
  "id":1
}
```

**实现**:
```rust
// domain/usecases/get_block.rs
pub struct GetBlockByHashUseCase {
    block_repo: Arc<dyn BlockRepository>,
    tx_repo: Arc<dyn TransactionRepository>,
}

impl GetBlockByHashUseCase {
    pub async fn execute(
        &self,
        block_hash: H256,
        full_transactions: bool,
    ) -> Result<Option<RichBlock>, UseCaseError> {
        // 1. 获取区块
        let block = self.block_repo.get_block_by_hash(block_hash).await?;

        let Some(block) = block else {
            return Ok(None);
        };

        // 2. 如果需要完整交易，加载交易详情
        let transactions = if full_transactions {
            let tx_hashes: Vec<_> = block.transactions.iter().map(|tx| tx.hash()).collect();
            let txs = self.tx_repo.get_transactions_by_hashes(&tx_hashes).await?;
            BlockTransactions::Full(txs)
        } else {
            let hashes = block.transactions.iter().map(|tx| tx.hash()).collect();
            BlockTransactions::Hashes(hashes)
        };

        Ok(Some(RichBlock {
            header: block.header,
            transactions,
            uncles: vec![],  // Post-merge: always empty
            withdrawals: block.withdrawals,
            size: block.size(),
        }))
    }
}
```

---

#### eth_getBlockByNumber

根据区块号返回区块信息。

**参数**:
1. `QUANTITY|TAG` - 区块号或区块标签
2. `Boolean` - 如果为 `true`，返回完整交易对象

**返回值**: `Object` - 区块对象（同 `eth_getBlockByHash`）

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getBlockByNumber",
  "params":["latest", false],
  "id":1
}
```

---

#### eth_getBlockTransactionCountByHash

返回指定区块中的交易数量。

**参数**:
1. `DATA`, 32 Bytes - 区块哈希

**返回值**: `QUANTITY` - 该区块中的交易数量

**示例**:
```json
{"jsonrpc":"2.0","id":1,"result":"0xa"}  // 10 transactions
```

---

#### eth_getBlockTransactionCountByNumber

根据区块号返回交易数量。

**参数**:
1. `QUANTITY|TAG` - 区块号或标签

**返回值**: `QUANTITY` - 交易数量

---

#### eth_getUncleCountByBlockHash

返回指定区块中的叔块数量。

**参数**:
1. `DATA`, 32 Bytes - 区块哈希

**返回值**: `QUANTITY` - 叔块数量

**注意**: Post-merge（合并后）总是返回 `"0x0"`

---

### 2.4 交易相关

#### eth_sendRawTransaction

提交已签名的交易。

**参数**:
1. `DATA` - 已签名的交易数据（RLP 编码）

**返回值**: `DATA`, 32 Bytes - 交易哈希

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_sendRawTransaction",
  "params":[
    "0xf86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83"
  ],
  "id":1
}

// 响应
{
  "jsonrpc":"2.0",
  "id":1,
  "result":"0xe670ec64341771606e55d6b4ca35a1a6b75ee3d5145a99d05921026d1527331"
}
```

**实现**:
```rust
// domain/usecases/submit_transaction.rs
pub struct SubmitRawTransactionUseCase {
    tx_pool: Arc<dyn TransactionPool>,
    tx_validator: Arc<dyn TransactionValidator>,
    p2p_service: Arc<dyn P2PService>,
}

impl SubmitRawTransactionUseCase {
    pub async fn execute(
        &self,
        raw_tx: Bytes,
    ) -> Result<TxHash, UseCaseError> {
        // 1. 解码交易
        let tx = Transaction::decode_signed(&raw_tx)
            .map_err(|_| UseCaseError::InvalidTransaction)?;

        // 2. 验证签名和基本字段
        self.tx_validator.validate(&tx)?;

        // 3. 恢复发送者地址
        let sender = tx.recover_sender()
            .ok_or(UseCaseError::InvalidSignature)?;

        // 4. 检查 nonce（防止重放攻击）
        let account_nonce = self.tx_pool.get_account_nonce(sender).await?;
        if tx.nonce < account_nonce {
            return Err(UseCaseError::NonceTooLow);
        }

        // 5. 添加到交易池
        let tx_hash = self.tx_pool.add(tx.clone()).await?;

        // 6. 广播到 P2P 网络
        self.p2p_service.broadcast_transactions(vec![tx]).await?;

        Ok(tx_hash)
    }
}

// interfaces/rpc/handlers/eth.rs
impl EthApiHandler {
    pub async fn send_raw_transaction(
        &self,
        raw_tx: Bytes,
    ) -> RpcResult<H256> {
        self.submit_tx_usecase
            .execute(raw_tx)
            .await
            .map_err(|e| match e {
                UseCaseError::NonceTooLow => RpcError::nonce_too_low(),
                UseCaseError::InsufficientFunds => RpcError::insufficient_funds(),
                UseCaseError::GasLimitTooHigh => RpcError::gas_limit_exceeded(),
                _ => RpcError::internal_error(e.to_string()),
            })
    }
}
```

---

#### eth_getTransactionByHash

根据交易哈希返回交易信息。

**参数**:
1. `DATA`, 32 Bytes - 交易哈希

**返回值**: `Object` - 交易对象，如果不存在返回 `null`

**交易对象结构**:
```json
{
  "blockHash": "0x1d59ff54b1eb26b013ce3cb5fc9dab3705b415a67127a003c3e61eb445bb8df2",
  "blockNumber": "0x5daf3b",
  "from": "0xa7d9ddbe1f17865597fbd27ec712455208b6b76d",
  "gas": "0xc350",
  "gasPrice": "0x4a817c800",
  "maxFeePerGas": "0x1229298c00",          // EIP-1559
  "maxPriorityFeePerGas": "0x49504f80",    // EIP-1559
  "hash": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
  "input": "0x68656c6c6f21",
  "nonce": "0x15",
  "to": "0xf02c1c8e6114b1dbe8937a39260b5b0a374432bb",
  "transactionIndex": "0x41",
  "value": "0xf3dbb76162000",
  "type": "0x2",                           // 0: Legacy, 1: EIP-2930, 2: EIP-1559, 3: EIP-4844
  "accessList": [],                        // EIP-2930
  "chainId": "0x1",
  "v": "0x25",
  "r": "0x1b5e176d927f8e9ab405058b2d2457392da3e20f328b16ddabcebc33eaac5fea",
  "s": "0x4ba69724e8f69de52f0125ad8b3c5c2cef33019bac3249e2c0a2192766d1721c",

  // EIP-4844 (Blob transactions)
  "maxFeePerBlobGas": "0x1",
  "blobVersionedHashes": ["0x..."]
}
```

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getTransactionByHash",
  "params":["0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"],
  "id":1
}
```

---

#### eth_getTransactionByBlockHashAndIndex

根据区块哈希和交易索引返回交易。

**参数**:
1. `DATA`, 32 Bytes - 区块哈希
2. `QUANTITY` - 交易索引位置

**返回值**: `Object` - 交易对象

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getTransactionByBlockHashAndIndex",
  "params":["0xe670ec64341771606e55d6b4ca35a1a6b75ee3d5145a99d05921026d1527331", "0x0"],
  "id":1
}
```

---

#### eth_getTransactionReceipt

返回交易收据。

**参数**:
1. `DATA`, 32 Bytes - 交易哈希

**返回值**: `Object` - 交易收据对象，如果交易未被打包返回 `null`

**收据对象结构**:
```json
{
  "transactionHash": "0xb903239f8543d04b5dc1ba6579132b143087c68db1b2168786408fcbce568238",
  "transactionIndex": "0x1",
  "blockHash": "0xc6ef2fc5426d6ad6fd9e2a26abeab0aa2411b7ab17f30a99d3cb96aed1d1055b",
  "blockNumber": "0xb",
  "from": "0xa7d9ddbe1f17865597fbd27ec712455208b6b76d",
  "to": "0xf02c1c8e6114b1dbe8937a39260b5b0a374432bb",
  "cumulativeGasUsed": "0x33bc",
  "effectiveGasPrice": "0x4a817c800",
  "gasUsed": "0x4dc",
  "contractAddress": null,      // 如果是合约创建交易，返回合约地址
  "logs": [                     // 事件日志数组
    {
      "logIndex": "0x1",
      "transactionIndex": "0x0",
      "transactionHash": "0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf",
      "blockHash": "0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d",
      "blockNumber": "0x1b4",
      "address": "0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d",
      "data": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "topics": [
        "0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5"
      ]
    }
  ],
  "logsBloom": "0x...",
  "status": "0x1",              // 1: success, 0: failure
  "type": "0x2",                // 交易类型

  // EIP-4844 (Blob transactions)
  "blobGasUsed": "0x20000",
  "blobGasPrice": "0x1"
}
```

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getTransactionReceipt",
  "params":["0xb903239f8543d04b5dc1ba6579132b143087c68db1b2168786408fcbce568238"],
  "id":1
}
```

**实现**:
```rust
pub struct GetTransactionReceiptUseCase {
    receipt_repo: Arc<dyn ReceiptRepository>,
    block_repo: Arc<dyn BlockRepository>,
}

impl GetTransactionReceiptUseCase {
    pub async fn execute(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, UseCaseError> {
        // 1. 查找收据
        let receipt = self.receipt_repo.get_receipt_by_tx_hash(tx_hash).await?;

        let Some(receipt) = receipt else {
            return Ok(None);
        };

        // 2. 获取区块信息计算 effectiveGasPrice
        let block = self.block_repo
            .get_block_by_hash(receipt.block_hash)
            .await?
            .ok_or(UseCaseError::BlockNotFound)?;

        let effective_gas_price = calculate_effective_gas_price(
            &receipt.transaction,
            block.base_fee_per_gas,
        );

        Ok(Some(TransactionReceipt {
            transaction_hash: receipt.transaction_hash,
            transaction_index: receipt.transaction_index,
            block_hash: receipt.block_hash,
            block_number: receipt.block_number,
            from: receipt.from,
            to: receipt.to,
            cumulative_gas_used: receipt.cumulative_gas_used,
            effective_gas_price,
            gas_used: receipt.gas_used,
            contract_address: receipt.contract_address,
            logs: receipt.logs,
            logs_bloom: receipt.logs_bloom,
            status: receipt.status,
            tx_type: receipt.tx_type,
            blob_gas_used: receipt.blob_gas_used,
            blob_gas_price: receipt.blob_gas_price,
        }))
    }
}
```

---

### 2.5 执行调用

#### eth_call

在不创建交易的情况下执行新的消息调用。

**参数**:
1. `Object` - 交易调用对象
   ```json
   {
     "from": "0x...",           // [可选] 发送者地址
     "to": "0x...",             // 目标地址
     "gas": "0x...",            // [可选] Gas 限制
     "gasPrice": "0x...",       // [可选] Gas 价格
     "value": "0x...",          // [可选] 转账金额
     "data": "0x..."            // [可选] 调用数据
   }
   ```
2. `QUANTITY|TAG` - 区块号或标签

**返回值**: `DATA` - 调用的返回值

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_call",
  "params":[
    {
      "to": "0xd46e8dd67c5d32be8058bb8eb970870f07244567",
      "data": "0x70a08231000000000000000000000000407d73d8a49eeb85d32cf465507dd71d507100c1"
    },
    "latest"
  ],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x0000000000000000000000000000000000000000000000000000000000000000"}
```

**实现**:
```rust
// domain/usecases/call_transaction.rs
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
    ) -> Result<Bytes, UseCaseError> {
        // 1. 解析区块
        let block_hash = self.block_resolver.resolve_block_id(block_id).await?;
        let block = self.state_db
            .get_block(block_hash)
            .await?
            .ok_or(UseCaseError::BlockNotFound)?;

        // 2. 获取只读状态
        let state = self.state_db.at_block(block_hash).await?;

        // 3. 构建 EVM 环境
        let env = EvmEnv {
            block_number: block.number,
            timestamp: block.timestamp,
            gas_limit: call.gas.unwrap_or(block.gas_limit),
            base_fee: block.base_fee_per_gas,
            coinbase: block.fee_recipient,
            difficulty: U256::ZERO,
            prevrandao: block.prev_randao,
            chain_id: self.evm.chain_id(),
        };

        // 4. 执行调用（只读，不修改状态）
        let result = self.evm.call(
            call.from.unwrap_or_default(),
            call.to.ok_or(UseCaseError::MissingTo)?,
            call.data.unwrap_or_default(),
            call.value.unwrap_or_default(),
            call.gas.unwrap_or(u64::MAX),
            state,
            env,
        ).await?;

        match result {
            ExecutionResult::Success { output, .. } => Ok(output),
            ExecutionResult::Revert { output, .. } => {
                Err(UseCaseError::ExecutionReverted(output))
            }
            ExecutionResult::Halt { reason, .. } => {
                Err(UseCaseError::ExecutionHalted(reason))
            }
        }
    }
}
```

---

#### eth_estimateGas

估算执行交易所需的 Gas。

**参数**:
1. `Object` - 交易调用对象（同 `eth_call`）
2. `QUANTITY|TAG` - [可选] 区块号或标签

**返回值**: `QUANTITY` - 估算的 Gas 用量

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_estimateGas",
  "params":[
    {
      "from": "0xb60e8dd61c5d32be8058bb8eb970870f07233155",
      "to": "0xd46e8dd67c5d32be8058bb8eb970870f07244567",
      "value": "0x9184e72a"
    }
  ],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x5208"}  // 21000 gas
```

**实现（二分搜索法）**:
```rust
pub struct EstimateGasUseCase {
    call_usecase: Arc<CallTransactionUseCase>,
    state_db: Arc<dyn StateRepository>,
}

impl EstimateGasUseCase {
    pub async fn execute(
        &self,
        mut call: CallRequest,
        block_id: BlockId,
    ) -> Result<u64, UseCaseError> {
        // 1. 获取账户余额用于设置上限
        let max_gas = if let Some(from) = call.from {
            let balance = self.get_balance(from, block_id).await?;
            std::cmp::min(
                balance / call.gas_price.unwrap_or(U256::from(1)),
                U256::from(30_000_000),
            ).as_u64()
        } else {
            30_000_000
        };

        let mut low = 21_000u64;  // 最小交易 gas
        let mut high = call.gas.unwrap_or(max_gas);

        // 2. 先测试上限是否足够
        call.gas = Some(high);
        if self.call_usecase.execute(call.clone(), block_id).await.is_err() {
            return Err(UseCaseError::ExecutionFailed);
        }

        // 3. 二分搜索找到最小 gas
        while low < high {
            let mid = (low + high) / 2;
            call.gas = Some(mid);

            match self.call_usecase.execute(call.clone(), block_id).await {
                Ok(_) => high = mid,
                Err(_) => low = mid + 1,
            }
        }

        // 4. 添加 5% 余量
        Ok(low * 105 / 100)
    }
}
```

---

### 2.6 过滤器和日志

#### eth_newFilter

创建一个过滤器对象，用于监听状态变化（日志）。

**参数**:
1. `Object` - 过滤器选项
   ```json
   {
     "fromBlock": "0x1",        // [可选] 起始区块
     "toBlock": "latest",       // [可选] 结束区块
     "address": "0x...",        // [可选] 合约地址或地址数组
     "topics": [                // [可选] 主题数组
       "0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b",
       null                     // null 表示匹配任何值
     ]
   }
   ```

**返回值**: `QUANTITY` - 过滤器 ID

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_newFilter",
  "params":[
    {
      "topics": ["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"]
    }
  ],
  "id":1
}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x1"}
```

---

#### eth_newBlockFilter

创建一个过滤器，监听新区块到达。

**参数**: 无

**返回值**: `QUANTITY` - 过滤器 ID

**示例**:
```json
{"jsonrpc":"2.0","method":"eth_newBlockFilter","params":[],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x1"}
```

---

#### eth_newPendingTransactionFilter

创建一个过滤器，监听待处理交易。

**参数**: 无

**返回值**: `QUANTITY` - 过滤器 ID

---

#### eth_uninstallFilter

卸载指定的过滤器。

**参数**:
1. `QUANTITY` - 过滤器 ID

**返回值**: `Boolean` - 如果成功卸载返回 `true`

**示例**:
```json
{"jsonrpc":"2.0","method":"eth_uninstallFilter","params":["0x1"],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":true}
```

---

#### eth_getFilterChanges

返回自上次查询以来的新日志。

**参数**:
1. `QUANTITY` - 过滤器 ID

**返回值**: `Array` - 日志对象数组，如果没有变化返回空数组

**示例**:
```json
{"jsonrpc":"2.0","method":"eth_getFilterChanges","params":["0x16"],"id":1}

// 响应
{
  "jsonrpc":"2.0",
  "id":1,
  "result": [
    {
      "logIndex": "0x1",
      "blockNumber": "0x1b4",
      "blockHash": "0x8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcfdf829c5a142f1fccd7d",
      "transactionHash": "0xdf829c5a142f1fccd7d8216c5785ac562ff41e2dcfdf5785ac562ff41e2dcf",
      "transactionIndex": "0x0",
      "address": "0x16c5785ac562ff41e2dcfdf829c5a142f1fccd7d",
      "data": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "topics": ["0x59ebeb90bc63057b6515673c3ecf9438e5058bca0f92585014eced636878c9a5"]
    }
  ]
}
```

**实现**:
```rust
pub struct FilterManager {
    filters: Arc<RwLock<HashMap<U256, Filter>>>,
    log_store: Arc<dyn LogRepository>,
}

pub enum Filter {
    Log {
        from_block: BlockNumber,
        to_block: BlockNumber,
        addresses: Vec<Address>,
        topics: Vec<Option<H256>>,
        last_poll_block: AtomicU64,
    },
    Block {
        last_poll_block: AtomicU64,
    },
    PendingTransaction {
        last_poll: AtomicU64,
    },
}

impl FilterManager {
    pub async fn get_filter_changes(
        &self,
        filter_id: U256,
    ) -> Result<FilterChanges, Error> {
        let filters = self.filters.read().await;
        let filter = filters.get(&filter_id).ok_or(Error::FilterNotFound)?;

        match filter {
            Filter::Log { last_poll_block, addresses, topics, .. } => {
                let last = last_poll_block.load(Ordering::Acquire);
                let current = self.get_current_block_number().await?;

                let logs = self.log_store.get_logs(
                    last + 1,
                    current,
                    addresses,
                    topics,
                ).await?;

                last_poll_block.store(current, Ordering::Release);

                Ok(FilterChanges::Logs(logs))
            }
            Filter::Block { last_poll_block } => {
                let last = last_poll_block.load(Ordering::Acquire);
                let current = self.get_current_block_number().await?;

                let block_hashes = self.get_block_hashes(last + 1, current).await?;

                last_poll_block.store(current, Ordering::Release);

                Ok(FilterChanges::Hashes(block_hashes))
            }
            Filter::PendingTransaction { last_poll } => {
                // 实现待处理交易过滤器
                todo!()
            }
        }
    }
}
```

---

#### eth_getFilterLogs

返回匹配指定过滤器的所有日志。

**参数**:
1. `QUANTITY` - 过滤器 ID

**返回值**: `Array` - 日志对象数组

---

#### eth_getLogs

根据过滤条件返回日志。

**参数**:
1. `Object` - 过滤器选项（同 `eth_newFilter`）

**返回值**: `Array` - 日志对象数组

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_getLogs",
  "params":[
    {
      "fromBlock": "0x1",
      "toBlock": "0x2",
      "address": "0x8888f1f195afa192cfee860698584c030f4c9db1",
      "topics": [
        "0x000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b"
      ]
    }
  ],
  "id":1
}
```

**性能优化**:
```rust
pub struct GetLogsUseCase {
    log_index: Arc<dyn LogIndexRepository>,
    block_repo: Arc<dyn BlockRepository>,
}

impl GetLogsUseCase {
    pub async fn execute(
        &self,
        filter: LogFilter,
    ) -> Result<Vec<Log>, UseCaseError> {
        // 1. 限制查询范围防止 DoS
        const MAX_BLOCK_RANGE: u64 = 10_000;
        let range = filter.to_block.saturating_sub(filter.from_block);
        if range > MAX_BLOCK_RANGE {
            return Err(UseCaseError::BlockRangeTooLarge);
        }

        // 2. 使用日志索引加速查询
        let logs = if filter.addresses.len() == 1 && filter.topics[0].is_some() {
            // 单个地址 + topic[0]: 使用复合索引
            self.log_index.get_logs_by_address_and_topic(
                filter.addresses[0],
                filter.topics[0].unwrap(),
                filter.from_block,
                filter.to_block,
            ).await?
        } else {
            // 通用查询
            self.log_index.get_logs_by_filter(&filter).await?
        };

        // 3. 过滤其他 topics
        let filtered = logs.into_iter()
            .filter(|log| self.matches_topics(log, &filter.topics))
            .collect();

        Ok(filtered)
    }

    fn matches_topics(&self, log: &Log, topics: &[Option<H256>]) -> bool {
        for (i, topic_filter) in topics.iter().enumerate() {
            if let Some(expected) = topic_filter {
                if log.topics.get(i) != Some(expected) {
                    return false;
                }
            }
        }
        true
    }
}
```

---

### 2.7 Gas 价格和费用

#### eth_gasPrice

返回当前的 gas 价格（单位：Wei）。

**参数**: 无

**返回值**: `QUANTITY` - Gas 价格

**示例**:
```json
{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}

// 响应
{"jsonrpc":"2.0","id":1,"result":"0x09184e72a000"}  // 10000000000000 Wei (10 Gwei)
```

**实现（基于历史数据）**:
```rust
pub struct GasPriceOracleUseCase {
    block_repo: Arc<dyn BlockRepository>,
    config: GasPriceOracleConfig,
}

pub struct GasPriceOracleConfig {
    pub sample_blocks: usize,
    pub percentile: f64,
    pub max_price: U256,
    pub min_price: U256,
}

impl GasPriceOracleUseCase {
    pub async fn execute(&self) -> Result<U256, UseCaseError> {
        // 1. 获取最近的区块
        let latest_block = self.block_repo.get_latest_block().await?;
        let start_block = latest_block.number.saturating_sub(self.config.sample_blocks as u64);

        // 2. 收集交易的 gas price
        let mut gas_prices = Vec::new();

        for block_num in start_block..=latest_block.number {
            let block = self.block_repo.get_block_by_number(block_num).await?;
            if let Some(block) = block {
                for tx in &block.transactions {
                    let gas_price = calculate_effective_gas_price(tx, block.base_fee_per_gas);
                    gas_prices.push(gas_price);
                }
            }
        }

        if gas_prices.is_empty() {
            // 如果没有交易，返回最新区块的 base fee
            return Ok(latest_block.base_fee_per_gas);
        }

        // 3. 计算百分位数
        gas_prices.sort_unstable();
        let index = ((gas_prices.len() - 1) as f64 * self.config.percentile) as usize;
        let suggested_price = gas_prices[index];

        // 4. 应用限制
        Ok(suggested_price.clamp(self.config.min_price, self.config.max_price))
    }
}

fn calculate_effective_gas_price(tx: &Transaction, base_fee: U256) -> U256 {
    match tx.transaction_type {
        TransactionType::Legacy => tx.gas_price,
        TransactionType::Eip2930 => tx.gas_price,
        TransactionType::Eip1559 => {
            let priority_fee = std::cmp::min(
                tx.max_priority_fee_per_gas,
                tx.max_fee_per_gas.saturating_sub(base_fee),
            );
            base_fee + priority_fee
        }
        TransactionType::Eip4844 => {
            let priority_fee = std::cmp::min(
                tx.max_priority_fee_per_gas,
                tx.max_fee_per_gas.saturating_sub(base_fee),
            );
            base_fee + priority_fee
        }
    }
}
```

---

#### eth_maxPriorityFeePerGas

返回建议的最大优先费用（EIP-1559）。

**参数**: 无

**返回值**: `QUANTITY` - 建议的优先费用

**示例**:
```json
{"jsonrpc":"2.0","id":1,"result":"0x59682f00"}  // 1.5 Gwei
```

---

#### eth_feeHistory

返回历史 gas 费用信息。

**参数**:
1. `QUANTITY` - 请求的区块数量
2. `QUANTITY|TAG` - 最新区块号或标签
3. `Array` - [可选] 请求的奖励百分位数数组

**返回值**: `Object` - 费用历史对象

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"eth_feeHistory",
  "params":["0x5", "latest", [25, 75]],
  "id":1
}

// 响应
{
  "jsonrpc":"2.0",
  "id":1,
  "result": {
    "oldestBlock": "0x1234567",
    "baseFeePerGas": [
      "0x12a05f200",
      "0x11e1a3000",
      "0x120c35200"
    ],
    "gasUsedRatio": [0.5, 0.6, 0.4],
    "reward": [
      ["0x59682f00", "0x77359400"],  // 25th, 75th percentile
      ["0x59682f00", "0x77359400"],
      ["0x59682f00", "0x77359400"]
    ]
  }
}
```

---

### 2.8 其他方法

#### eth_chainId

返回当前链的 ID（EIP-155）。

**参数**: 无

**返回值**: `QUANTITY` - 链 ID

**示例**:
```json
{"jsonrpc":"2.0","id":1,"result":"0x1"}  // Mainnet
```

---

#### eth_syncing

返回同步状态。

**参数**: 无

**返回值**: `Object|Boolean` - 如果正在同步返回同步状态对象，否则返回 `false`

**同步状态对象**:
```json
{
  "startingBlock": "0x384",
  "currentBlock": "0x386",
  "highestBlock": "0x454"
}
```

**示例**:
```json
// 正在同步
{"jsonrpc":"2.0","id":1,"result":{"startingBlock":"0x0","currentBlock":"0x1","highestBlock":"0x100"}}

// 同步完成
{"jsonrpc":"2.0","id":1,"result":false}
```

**实现**:
```rust
pub struct SyncingStatusUseCase {
    sync_service: Arc<dyn SyncService>,
}

impl SyncingStatusUseCase {
    pub async fn execute(&self) -> Result<SyncingStatus, UseCaseError> {
        let status = self.sync_service.get_status().await?;

        if status.is_syncing {
            Ok(SyncingStatus::Syncing {
                starting_block: status.starting_block,
                current_block: status.current_block,
                highest_block: status.highest_block,
            })
        } else {
            Ok(SyncingStatus::NotSyncing)
        }
    }
}
```

---

#### eth_coinbase

返回客户端的 coinbase 地址（挖矿奖励地址）。

**参数**: 无

**返回值**: `DATA`, 20 Bytes - Coinbase 地址

**注意**: Post-merge（合并后），这通常是 fee recipient 地址。

---

#### eth_mining

返回客户端是否正在挖矿。

**参数**: 无

**返回值**: `Boolean` - 是否正在挖矿

**注意**: Post-merge 总是返回 `false`。

---

#### eth_hashrate

返回节点正在挖矿的哈希率。

**参数**: 无

**返回值**: `QUANTITY` - 哈希率（hashes/second）

**注意**: Post-merge 总是返回 `"0x0"`。

---

## 3. Engine API (共识层通信)

Engine API 用于共识客户端与执行客户端之间的通信。

### 3.1 engine_newPayloadV3

接收并验证新的执行载荷。

**参数**:
1. `Object` - 执行载荷对象
2. `Array<DATA>` - 预期的 blob 版本化哈希（EIP-4844）
3. `DATA` - 父信标块根（EIP-4788）

**执行载荷对象**:
```json
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
  "withdrawals": [
    {
      "index": "0x0",
      "validatorIndex": "0x1",
      "address": "0x...",
      "amount": "0x64"
    }
  ],
  "blobGasUsed": "0x20000",
  "excessBlobGas": "0x0"
}
```

**返回值**: `Object` - 载荷状态对象
```json
{
  "status": "VALID",          // VALID, INVALID, SYNCING, ACCEPTED
  "latestValidHash": "0x...",
  "validationError": null
}
```

**实现**:
```rust
// domain/usecases/engine/new_payload.rs
pub struct NewPayloadUseCase {
    state_db: Arc<dyn StateRepository>,
    tx_executor: Arc<dyn TransactionExecutor>,
    block_validator: Arc<dyn BlockValidator>,
    block_repo: Arc<dyn BlockRepository>,
}

impl NewPayloadUseCase {
    pub async fn execute(
        &self,
        payload: ExecutionPayload,
        expected_blob_hashes: Vec<H256>,
        parent_beacon_root: H256,
    ) -> Result<PayloadStatus, UseCaseError> {
        // 1. 基本验证
        if !self.block_repo.has_block(payload.parent_hash).await? {
            return Ok(PayloadStatus::Syncing);
        }

        // 2. 验证区块头
        self.block_validator.validate_header(&payload.header)?;

        // 3. 验证 blob 哈希（EIP-4844）
        self.validate_blob_hashes(&payload, &expected_blob_hashes)?;

        // 4. 创建状态快照
        let mut state = self.state_db.at_block(payload.parent_hash).await?;

        // 5. 存储 parent beacon root（EIP-4788）
        self.store_beacon_root(&mut state, parent_beacon_root, payload.timestamp)?;

        // 6. 执行交易
        let mut receipts = Vec::new();
        let mut cumulative_gas = 0u64;
        let mut logs_bloom = Bloom::default();

        for (idx, tx_bytes) in payload.transactions.iter().enumerate() {
            let tx = Transaction::decode_signed(tx_bytes)
                .map_err(|_| UseCaseError::InvalidTransaction(idx))?;

            let receipt = self.tx_executor.execute(
                &tx,
                &mut state,
                &payload.header,
                cumulative_gas,
            ).await?;

            cumulative_gas += receipt.gas_used;
            logs_bloom.accrue_bloom(&receipt.logs_bloom);
            receipts.push(receipt);
        }

        // 7. 处理提款（Withdrawals）
        for withdrawal in &payload.withdrawals {
            state.add_balance(
                withdrawal.address,
                U256::from(withdrawal.amount) * U256::from(1_000_000_000), // Gwei to Wei
            )?;
        }

        // 8. 验证状态根
        let computed_state_root = state.compute_root()?;
        if computed_state_root != payload.state_root {
            return Ok(PayloadStatus::Invalid {
                latest_valid_hash: Some(payload.parent_hash),
                validation_error: Some("Invalid state root".into()),
            });
        }

        // 9. 验证 receipts root
        let receipts_root = calculate_receipts_root(&receipts);
        if receipts_root != payload.receipts_root {
            return Ok(PayloadStatus::Invalid {
                latest_valid_hash: Some(payload.parent_hash),
                validation_error: Some("Invalid receipts root".into()),
            });
        }

        // 10. 验证 logs bloom
        if logs_bloom != payload.logs_bloom {
            return Ok(PayloadStatus::Invalid {
                latest_valid_hash: Some(payload.parent_hash),
                validation_error: Some("Invalid logs bloom".into()),
            });
        }

        // 11. 验证 gas used
        if cumulative_gas != payload.gas_used {
            return Ok(PayloadStatus::Invalid {
                latest_valid_hash: Some(payload.parent_hash),
                validation_error: Some("Invalid gas used".into()),
            });
        }

        // 12. 持久化状态和区块
        self.state_db.commit(state).await?;
        self.block_repo.insert_block(payload.into_block(receipts)).await?;

        Ok(PayloadStatus::Valid {
            latest_valid_hash: payload.block_hash,
        })
    }

    fn store_beacon_root(
        &self,
        state: &mut State,
        beacon_root: H256,
        timestamp: u64,
    ) -> Result<(), UseCaseError> {
        // EIP-4788: 存储 parent beacon root
        const BEACON_ROOTS_ADDRESS: Address =
            Address::from_low_u64_be(0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02);

        let timestamp_idx = U256::from(timestamp % 8191);
        let root_idx = timestamp_idx + U256::from(8191);

        state.set_storage(BEACON_ROOTS_ADDRESS, timestamp_idx, U256::from(timestamp))?;
        state.set_storage(BEACON_ROOTS_ADDRESS, root_idx, U256::from(beacon_root))?;

        Ok(())
    }

    fn validate_blob_hashes(
        &self,
        payload: &ExecutionPayload,
        expected: &[H256],
    ) -> Result<(), UseCaseError> {
        // 从 blob transactions 中提取 blob hashes
        let mut actual_hashes = Vec::new();

        for tx_bytes in &payload.transactions {
            let tx = Transaction::decode_signed(tx_bytes)?;
            if let TransactionType::Eip4844 = tx.transaction_type {
                actual_hashes.extend_from_slice(&tx.blob_versioned_hashes);
            }
        }

        if actual_hashes != expected {
            return Err(UseCaseError::InvalidBlobHashes);
        }

        Ok(())
    }
}

// interfaces/rpc/handlers/engine.rs
pub struct EngineApiHandler {
    new_payload: Arc<NewPayloadUseCase>,
}

impl EngineApiHandler {
    pub async fn new_payload_v3(
        &self,
        payload: ExecutionPayloadV3,
        expected_blob_versioned_hashes: Vec<B256>,
        parent_beacon_block_root: B256,
    ) -> EngineResult<PayloadStatusV1> {
        self.new_payload
            .execute(payload.into(), expected_blob_versioned_hashes, parent_beacon_block_root)
            .await
            .map_err(|e| EngineError::internal(e.to_string()))
    }
}
```

---

### 3.2 engine_forkchoiceUpdatedV3

更新分叉选择状态。

**参数**:
1. `Object` - Forkchoice 状态对象
   ```json
   {
     "headBlockHash": "0x...",
     "safeBlockHash": "0x...",
     "finalizedBlockHash": "0x..."
   }
   ```
2. `Object|null` - [可选] 载荷属性对象
   ```json
   {
     "timestamp": "0x6543210",
     "prevRandao": "0x...",
     "suggestedFeeRecipient": "0x...",
     "withdrawals": [...],
     "parentBeaconBlockRoot": "0x..."
   }
   ```

**返回值**: `Object` - Forkchoice 更新结果
```json
{
  "payloadStatus": {
    "status": "VALID",
    "latestValidHash": "0x...",
    "validationError": null
  },
  "payloadId": "0x1234567890abcdef"  // 如果请求构建新 payload
}
```

**实现**:
```rust
// domain/usecases/engine/forkchoice_updated.rs
pub struct ForkchoiceUpdatedUseCase {
    chain_state: Arc<dyn ChainStateRepository>,
    payload_builder: Arc<dyn PayloadBuilder>,
    block_repo: Arc<dyn BlockRepository>,
}

impl ForkchoiceUpdatedUseCase {
    pub async fn execute(
        &self,
        forkchoice: ForkchoiceState,
        attrs: Option<PayloadAttributes>,
    ) -> Result<ForkchoiceUpdatedResponse, UseCaseError> {
        // 1. 验证所有区块哈希存在
        if !self.block_repo.has_block(forkchoice.head_block_hash).await? {
            return Ok(ForkchoiceUpdatedResponse {
                payload_status: PayloadStatus::Syncing,
                payload_id: None,
            });
        }

        // 2. 更新链头
        self.chain_state.set_head(forkchoice.head_block_hash).await?;

        // 3. 标记安全区块
        if forkchoice.safe_block_hash != H256::zero() {
            self.chain_state.set_safe(forkchoice.safe_block_hash).await?;
        }

        // 4. 标记最终确定区块
        if forkchoice.finalized_block_hash != H256::zero() {
            self.chain_state.set_finalized(forkchoice.finalized_block_hash).await?;

            // 清理最终确定块之前的分叉
            self.prune_old_forks(forkchoice.finalized_block_hash).await?;
        }

        // 5. 如果需要构建新 payload
        let payload_id = if let Some(attrs) = attrs {
            // 启动后台 payload 构建
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

    async fn prune_old_forks(&self, finalized_hash: H256) -> Result<(), UseCaseError> {
        // 删除不在主链上的区块
        self.block_repo.prune_non_canonical_blocks(finalized_hash).await?;
        Ok(())
    }
}

// infrastructure/payload_builder.rs
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

        // 在后台构建 payload
        let builder = self.clone();
        tokio::spawn(async move {
            let payload = builder.build_payload(parent, attrs).await;
            if let Ok(payload) = payload {
                let mut payloads = builder.payloads.write().await;
                payloads.insert(payload_id, payload);
            }
        });

        Ok(payload_id)
    }

    async fn build_payload(
        &self,
        parent: H256,
        attrs: PayloadAttributes,
    ) -> Result<BuiltPayload, Error> {
        // 1. 获取父状态
        let mut state = self.state_db.at_block(parent).await?;
        let parent_block = self.state_db.get_block(parent).await?.unwrap();

        // 2. 从交易池选择交易
        let gas_limit = parent_block.gas_limit;
        let base_fee = calculate_next_base_fee(
            parent_block.gas_used,
            parent_block.gas_limit,
            parent_block.base_fee_per_gas,
        );

        let txs = self.tx_pool.select_transactions(
            gas_limit,
            |tx| {
                match tx.transaction_type {
                    TransactionType::Legacy => tx.gas_price >= base_fee,
                    TransactionType::Eip1559 | TransactionType::Eip4844 => {
                        tx.max_fee_per_gas >= base_fee
                    }
                    _ => true,
                }
            }
        ).await?;

        // 3. 执行交易
        let mut encoded_txs = Vec::new();
        let mut cumulative_gas = 0u64;
        let mut receipts = Vec::new();
        let mut total_fees = U256::ZERO;

        for tx in txs {
            if cumulative_gas + tx.gas_limit > gas_limit {
                break;
            }

            match self.tx_executor.execute(&tx, &mut state, &attrs, cumulative_gas).await {
                Ok(receipt) => {
                    cumulative_gas += receipt.gas_used;
                    total_fees += U256::from(receipt.gas_used) * calculate_effective_gas_price(&tx, base_fee);
                    encoded_txs.push(tx.encode_signed());
                    receipts.push(receipt);
                }
                Err(_) => {
                    // 跳过失败的交易
                    continue;
                }
            }
        }

        // 4. 处理提款
        for withdrawal in &attrs.withdrawals {
            state.add_balance(
                withdrawal.address,
                U256::from(withdrawal.amount) * U256::from(1_000_000_000),
            )?;
        }

        // 5. 计算状态根
        let state_root = state.compute_root()?;
        let receipts_root = calculate_receipts_root(&receipts);
        let withdrawals_root = calculate_withdrawals_root(&attrs.withdrawals);

        // 6. 构建 payload
        let payload = ExecutionPayload {
            parent_hash: parent,
            fee_recipient: attrs.suggested_fee_recipient,
            state_root,
            receipts_root,
            logs_bloom: calculate_logs_bloom(&receipts),
            prev_randao: attrs.prev_randao,
            block_number: parent_block.number + 1,
            gas_limit,
            gas_used: cumulative_gas,
            timestamp: attrs.timestamp,
            extra_data: Bytes::new(),
            base_fee_per_gas: base_fee,
            block_hash: H256::zero(), // 稍后计算
            transactions: encoded_txs,
            withdrawals: attrs.withdrawals,
            blob_gas_used: calculate_blob_gas_used(&receipts),
            excess_blob_gas: calculate_excess_blob_gas(parent_block.excess_blob_gas, parent_block.blob_gas_used),
        };

        // 7. 计算区块哈希
        let block_hash = payload.compute_block_hash();
        let payload = ExecutionPayload { block_hash, ..payload };

        Ok(BuiltPayload {
            execution_payload: payload,
            block_value: total_fees,
            blobs_bundle: None, // 如果有 blob transactions，填充此字段
        })
    }
}
```

---

### 3.3 engine_getPayloadV3

获取已构建的执行载荷。

**参数**:
1. `DATA`, 8 Bytes - Payload ID

**返回值**: `Object` - 获取 payload 响应
```json
{
  "executionPayload": { ... },
  "blockValue": "0x1234567890",
  "blobsBundle": {
    "commitments": ["0x..."],
    "proofs": ["0x..."],
    "blobs": ["0x..."]
  },
  "shouldOverrideBuilder": false
}
```

**实现**:
```rust
pub struct GetPayloadUseCase {
    payload_store: Arc<dyn PayloadStore>,
}

impl GetPayloadUseCase {
    pub async fn execute(
        &self,
        payload_id: PayloadId,
    ) -> Result<GetPayloadResponse, UseCaseError> {
        let payload = self.payload_store
            .get_payload(payload_id)
            .await?
            .ok_or(UseCaseError::PayloadNotFound)?;

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

### 3.4 engine_getPayloadBodiesByHashV1

根据区块哈希获取执行载荷主体。

**参数**:
1. `Array<DATA>` - 区块哈希数组

**返回值**: `Array<Object|null>` - 载荷主体数组

---

### 3.5 engine_getPayloadBodiesByRangeV1

根据区块范围获取执行载荷主体。

**参数**:
1. `QUANTITY` - 起始区块号
2. `QUANTITY` - 区块数量

**返回值**: `Array<Object|null>` - 载荷主体数组

---

### 3.6 engine_exchangeCapabilities

交换客户端能力信息。

**参数**:
1. `Array<String>` - 客户端支持的能力列表

**返回值**: `Array<String>` - 服务端支持的能力列表

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"engine_exchangeCapabilities",
  "params":[
    [
      "engine_newPayloadV3",
      "engine_forkchoiceUpdatedV3",
      "engine_getPayloadV3"
    ]
  ],
  "id":1
}

// 响应
{
  "jsonrpc":"2.0",
  "id":1,
  "result": [
    "engine_newPayloadV1",
    "engine_newPayloadV2",
    "engine_newPayloadV3",
    "engine_forkchoiceUpdatedV1",
    "engine_forkchoiceUpdatedV2",
    "engine_forkchoiceUpdatedV3",
    "engine_getPayloadV1",
    "engine_getPayloadV2",
    "engine_getPayloadV3"
  ]
}
```

---

## 4. Admin & Debug API

### 4.1 Admin API

用于节点管理的管理员 API。

#### admin_nodeInfo

返回节点信息。

**参数**: 无

**返回值**: `Object` - 节点信息

```json
{
  "id": "44826a5d6a55f88a18298bca4773fca5104cdf438e8ef2792495f3",
  "name": "Reth/v1.0.0/linux-x86_64/rustc1.75.0",
  "enode": "enode://44826a5d6a55f88a18298bca4773fca5104cdf438e8ef2792495f3@127.0.0.1:30303",
  "enr": "enr:-...",
  "ip": "127.0.0.1",
  "ports": {
    "discovery": 30303,
    "listener": 30303
  },
  "listenAddr": "[::]:30303",
  "protocols": {
    "eth": {
      "network": 1,
      "difficulty": 0,
      "genesis": "0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3",
      "config": {...},
      "head": "0x..."
    }
  }
}
```

---

#### admin_peers

返回连接的对等节点列表。

**参数**: 无

**返回值**: `Array<Object>` - 对等节点信息数组

---

#### admin_addPeer

添加新的对等节点。

**参数**:
1. `String` - enode URL

**返回值**: `Boolean` - 是否成功添加

**示例**:
```json
{
  "jsonrpc":"2.0",
  "method":"admin_addPeer",
  "params":["enode://...@192.168.1.100:30303"],
  "id":1
}
```

---

#### admin_removePeer

移除对等节点。

**参数**:
1. `String` - enode URL

**返回值**: `Boolean` - 是否成功移除

---

### 4.2 Debug API

用于调试和开发的 API。

#### debug_traceTransaction

跟踪交易执行过程。

**参数**:
1. `DATA`, 32 Bytes - 交易哈希
2. `Object` - [可选] 跟踪配置

**跟踪配置**:
```json
{
  "tracer": "callTracer",       // 跟踪器类型
  "tracerConfig": {
    "onlyTopCall": false,
    "withLog": true
  },
  "timeout": "5s",
  "disableStorage": false,
  "disableStack": false,
  "enableMemory": false,
  "enableReturnData": true
}
```

**返回值**: `Object` - 跟踪结果（格式取决于跟踪器）

**示例（callTracer）**:
```json
{
  "jsonrpc":"2.0",
  "id":1,
  "result": {
    "from": "0x...",
    "gas": "0x...",
    "gasUsed": "0x...",
    "to": "0x...",
    "input": "0x...",
    "output": "0x...",
    "value": "0x0",
    "type": "CALL",
    "calls": [
      {
        "from": "0x...",
        "gas": "0x...",
        "gasUsed": "0x...",
        "to": "0x...",
        "input": "0x...",
        "output": "0x...",
        "type": "DELEGATECALL"
      }
    ]
  }
}
```

**实现**:
```rust
// domain/usecases/debug/trace_transaction.rs
pub struct TraceTransactionUseCase {
    block_repo: Arc<dyn BlockRepository>,
    tx_repo: Arc<dyn TransactionRepository>,
    state_db: Arc<dyn StateRepository>,
    evm: Arc<dyn EvmExecutor>,
}

impl TraceTransactionUseCase {
    pub async fn execute(
        &self,
        tx_hash: H256,
        config: TraceConfig,
    ) -> Result<TraceResult, UseCaseError> {
        // 1. 查找交易
        let tx = self.tx_repo
            .get_transaction_by_hash(tx_hash)
            .await?
            .ok_or(UseCaseError::TransactionNotFound)?;

        // 2. 获取交易所在区块
        let block = self.block_repo
            .get_block_by_hash(tx.block_hash)
            .await?
            .ok_or(UseCaseError::BlockNotFound)?;

        // 3. 重放区块直到目标交易
        let mut state = self.state_db.at_block(block.parent_hash).await?;

        let mut cumulative_gas = 0u64;
        for block_tx in &block.transactions {
            if block_tx.hash() == tx_hash {
                // 找到目标交易，启用跟踪执行
                return self.trace_execution(
                    &tx,
                    &mut state,
                    &block.header,
                    cumulative_gas,
                    config,
                ).await;
            }

            // 执行前面的交易以获得正确的状态
            let receipt = self.evm.execute(
                block_tx,
                &mut state,
                &block.header,
                cumulative_gas,
            ).await?;

            cumulative_gas += receipt.gas_used;
        }

        Err(UseCaseError::TransactionNotFound)
    }

    async fn trace_execution(
        &self,
        tx: &Transaction,
        state: &mut State,
        block_header: &Header,
        cumulative_gas: u64,
        config: TraceConfig,
    ) -> Result<TraceResult, UseCaseError> {
        // 根据配置选择跟踪器
        let tracer: Box<dyn Tracer> = match config.tracer.as_deref() {
            Some("callTracer") => Box::new(CallTracer::new(config.tracer_config)),
            Some("prestateTracer") => Box::new(PrestateTracer::new()),
            _ => Box::new(StructLogTracer::new(config)),
        };

        // 执行并跟踪
        let result = self.evm.execute_with_tracer(
            tx,
            state,
            block_header,
            cumulative_gas,
            tracer,
        ).await?;

        Ok(result)
    }
}
```

---

#### debug_traceBlockByHash

跟踪整个区块的执行。

**参数**:
1. `DATA`, 32 Bytes - 区块哈希
2. `Object` - [可选] 跟踪配置

**返回值**: `Array<Object>` - 该区块所有交易的跟踪结果

---

#### debug_traceCall

跟踪调用执行（类似 `eth_call` 但返回跟踪信息）。

**参数**:
1. `Object` - 交易调用对象
2. `QUANTITY|TAG` - 区块号或标签
3. `Object` - [可选] 跟踪配置

**返回值**: `Object` - 跟踪结果

---

#### debug_getRawBlock

返回 RLP 编码的区块数据。

**参数**:
1. `QUANTITY|TAG` - 区块号或标签

**返回值**: `DATA` - RLP 编码的区块

---

#### debug_getRawTransaction

返回 RLP 编码的交易数据。

**参数**:
1. `DATA`, 32 Bytes - 交易哈希

**返回值**: `DATA` - RLP 编码的交易

---

#### debug_getRawReceipts

返回指定区块的原始收据数据。

**参数**:
1. `QUANTITY|TAG` - 区块号或标签

**返回值**: `Array<DATA>` - RLP 编码的收据数组

---

### 4.3 TxPool API

#### txpool_status

返回交易池状态。

**参数**: 无

**返回值**: `Object` - 交易池状态

```json
{
  "pending": "0xa",   // 10 pending transactions
  "queued": "0x5"     // 5 queued transactions
}
```

---

#### txpool_inspect

返回交易池中交易的摘要。

**参数**: 无

**返回值**: `Object` - 交易摘要

```json
{
  "pending": {
    "0xAddress1": {
      "0": "0xAddress2: 1 wei + 21000 gas × 1000000000 wei",
      "1": "0xAddress3: 2 wei + 21000 gas × 1000000000 wei"
    }
  },
  "queued": {
    "0xAddress4": {
      "3": "0xAddress5: 10 wei + 21000 gas × 1000000000 wei"
    }
  }
}
```

---

#### txpool_content

返回交易池中所有交易的详细信息。

**参数**: 无

**返回值**: `Object` - 完整的交易对象

---

## 5. Clean Architecture 实现

### 5.1 分层架构示例

```rust
// 项目结构
src/
├── domain/                    # 领域层（核心业务逻辑）
│   ├── entities/             # 实体和值对象
│   │   ├── block.rs
│   │   ├── transaction.rs
│   │   ├── account.rs
│   │   └── receipt.rs
│   ├── usecases/             # 用例实现
│   │   ├── get_balance.rs
│   │   ├── submit_transaction.rs
│   │   ├── call_transaction.rs
│   │   └── engine/
│   │       ├── new_payload.rs
│   │       └── forkchoice_updated.rs
│   └── repositories.rs       # 仓储接口定义
│
├── interfaces/               # 接口适配层
│   └── rpc/
│       ├── handlers/
│       │   ├── eth.rs        # eth_* 方法处理器
│       │   ├── engine.rs     # engine_* 方法处理器
│       │   ├── debug.rs      # debug_* 方法处理器
│       │   └── admin.rs      # admin_* 方法处理器
│       ├── dto.rs            # DTO 数据传输对象
│       ├── error.rs          # RPC 错误处理
│       └── server.rs         # RPC 服务器
│
└── infrastructure/           # 基础设施层
    ├── repositories/        # 仓储实现
    │   ├── state_db.rs
    │   ├── block_db.rs
    │   └── tx_pool.rs
    ├── evm/                 # EVM 执行引擎
    │   └── revm.rs
    └── p2p/                 # P2P 网络
        └── devp2p.rs
```

---

### 5.2 依赖注入

```rust
// main.rs - 应用入口和依赖注入
use std::sync::Arc;

pub struct AppContext {
    // Repositories (Infrastructure Layer)
    pub state_db: Arc<dyn StateRepository>,
    pub block_repo: Arc<dyn BlockRepository>,
    pub tx_pool: Arc<dyn TransactionPool>,
    pub receipt_repo: Arc<dyn ReceiptRepository>,

    // Services (Infrastructure Layer)
    pub evm: Arc<dyn EvmExecutor>,
    pub p2p: Arc<dyn P2PService>,
    pub block_validator: Arc<dyn BlockValidator>,

    // Use Cases (Application Layer)
    pub get_balance: Arc<GetBalanceUseCase>,
    pub submit_tx: Arc<SubmitRawTransactionUseCase>,
    pub call_tx: Arc<CallTransactionUseCase>,
    pub estimate_gas: Arc<EstimateGasUseCase>,
    pub get_block: Arc<GetBlockByHashUseCase>,
    pub new_payload: Arc<NewPayloadUseCase>,
    pub forkchoice_updated: Arc<ForkchoiceUpdatedUseCase>,

    // Handlers (Interface Layer)
    pub eth_handler: Arc<EthApiHandler>,
    pub engine_handler: Arc<EngineApiHandler>,
}

impl AppContext {
    pub fn new(config: Config) -> Self {
        // === Infrastructure Layer ===
        let db = Arc::new(RocksDB::open(&config.db_path).unwrap());

        let state_db: Arc<dyn StateRepository> = Arc::new(
            MPTStateDatabase::new(db.clone())
        );

        let block_repo: Arc<dyn BlockRepository> = Arc::new(
            BlockRepositoryImpl::new(db.clone())
        );

        let tx_pool: Arc<dyn TransactionPool> = Arc::new(
            TransactionPoolService::new(state_db.clone(), config.tx_pool)
        );

        let receipt_repo: Arc<dyn ReceiptRepository> = Arc::new(
            ReceiptRepositoryImpl::new(db.clone())
        );

        let evm: Arc<dyn EvmExecutor> = Arc::new(
            ReVmExecutor::new(config.chain_id)
        );

        let p2p: Arc<dyn P2PService> = Arc::new(
            DevP2PService::new(config.p2p)
        );

        let block_validator: Arc<dyn BlockValidator> = Arc::new(
            BlockValidatorService::new(config.consensus_params)
        );

        let block_resolver: Arc<dyn BlockResolver> = Arc::new(
            BlockResolverService::new(block_repo.clone())
        );

        // === Use Cases ===
        let get_balance = Arc::new(GetBalanceUseCase {
            state_db: state_db.clone(),
            block_resolver: block_resolver.clone(),
        });

        let tx_validator = Arc::new(TransactionValidatorService::new());

        let submit_tx = Arc::new(SubmitRawTransactionUseCase {
            tx_pool: tx_pool.clone(),
            tx_validator: tx_validator.clone(),
            p2p_service: p2p.clone(),
        });

        let call_tx = Arc::new(CallTransactionUseCase {
            state_db: state_db.clone(),
            evm: evm.clone(),
            block_resolver: block_resolver.clone(),
        });

        let estimate_gas = Arc::new(EstimateGasUseCase {
            call_usecase: call_tx.clone(),
            state_db: state_db.clone(),
        });

        let get_block = Arc::new(GetBlockByHashUseCase {
            block_repo: block_repo.clone(),
            tx_repo: Arc::new(TransactionRepositoryImpl::new(db.clone())),
        });

        let tx_executor: Arc<dyn TransactionExecutor> = Arc::new(
            TransactionExecutorService::new(evm.clone())
        );

        let new_payload = Arc::new(NewPayloadUseCase {
            state_db: state_db.clone(),
            tx_executor: tx_executor.clone(),
            block_validator: block_validator.clone(),
            block_repo: block_repo.clone(),
        });

        let payload_builder: Arc<dyn PayloadBuilder> = Arc::new(
            PayloadBuilderService::new(
                tx_pool.clone(),
                state_db.clone(),
                tx_executor.clone(),
            )
        );

        let chain_state: Arc<dyn ChainStateRepository> = Arc::new(
            ChainStateRepositoryImpl::new(db.clone())
        );

        let forkchoice_updated = Arc::new(ForkchoiceUpdatedUseCase {
            chain_state: chain_state.clone(),
            payload_builder: payload_builder.clone(),
            block_repo: block_repo.clone(),
        });

        // === Interface Layer ===
        let eth_handler = Arc::new(EthApiHandler {
            get_balance: get_balance.clone(),
            submit_tx: submit_tx.clone(),
            call_tx: call_tx.clone(),
            estimate_gas: estimate_gas.clone(),
            get_block: get_block.clone(),
            // ... 其他用例
        });

        let engine_handler = Arc::new(EngineApiHandler {
            new_payload: new_payload.clone(),
            forkchoice_updated: forkchoice_updated.clone(),
            // ... 其他用例
        });

        Self {
            state_db,
            block_repo,
            tx_pool,
            receipt_repo,
            evm,
            p2p,
            block_validator,
            get_balance,
            submit_tx,
            call_tx,
            estimate_gas,
            get_block,
            new_payload,
            forkchoice_updated,
            eth_handler,
            engine_handler,
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config::from_file("config.toml").unwrap();
    let ctx = Arc::new(AppContext::new(config.clone()));

    // 启动 JSON-RPC 服务器
    let rpc_server = RpcServer::new(config.rpc);
    rpc_server.register_handler("eth", ctx.eth_handler.clone());
    rpc_server.register_handler("engine", ctx.engine_handler.clone());

    tokio::spawn(rpc_server.start());

    // 启动 P2P 网络
    tokio::spawn(ctx.p2p.start());

    // 等待退出信号
    tokio::signal::ctrl_c().await.unwrap();
}
```

---

### 5.3 接口层实现

```rust
// interfaces/rpc/handlers/eth.rs
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

#[rpc(server, namespace = "eth")]
pub trait EthApi {
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> RpcResult<U64>;

    #[method(name = "getBalance")]
    async fn get_balance(
        &self,
        address: Address,
        block: BlockNumberOrTag,
    ) -> RpcResult<U256>;

    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, tx: Bytes) -> RpcResult<H256>;

    #[method(name = "call")]
    async fn call(
        &self,
        call: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<Bytes>;

    #[method(name = "estimateGas")]
    async fn estimate_gas(
        &self,
        call: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256>;
}

pub struct EthApiHandler {
    pub get_balance: Arc<GetBalanceUseCase>,
    pub submit_tx: Arc<SubmitRawTransactionUseCase>,
    pub call_tx: Arc<CallTransactionUseCase>,
    pub estimate_gas: Arc<EstimateGasUseCase>,
    pub block_number_cache: Arc<AtomicU64>,
}

#[async_trait]
impl EthApiServer for EthApiHandler {
    async fn block_number(&self) -> RpcResult<U64> {
        let number = self.block_number_cache.load(Ordering::Acquire);
        Ok(U64::from(number))
    }

    async fn get_balance(
        &self,
        address: Address,
        block: BlockNumberOrTag,
    ) -> RpcResult<U256> {
        let block_id = self.resolve_block_id(block)?;

        self.get_balance
            .execute(address, block_id)
            .await
            .map_err(|e| RpcError::internal(e.to_string()))
    }

    async fn send_raw_transaction(&self, raw_tx: Bytes) -> RpcResult<H256> {
        self.submit_tx
            .execute(raw_tx)
            .await
            .map_err(|e| match e {
                UseCaseError::NonceTooLow => RpcError::nonce_too_low(),
                UseCaseError::InsufficientFunds => RpcError::insufficient_funds(),
                UseCaseError::GasLimitTooHigh => RpcError::gas_limit_exceeded(),
                _ => RpcError::internal(e.to_string()),
            })
    }

    async fn call(
        &self,
        call: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<Bytes> {
        let block_id = self.resolve_block_id(block.unwrap_or(BlockNumberOrTag::Latest))?;

        self.call_tx
            .execute(call, block_id)
            .await
            .map_err(|e| match e {
                UseCaseError::ExecutionReverted(data) => RpcError::revert(data),
                _ => RpcError::internal(e.to_string()),
            })
    }

    async fn estimate_gas(
        &self,
        call: CallRequest,
        block: Option<BlockNumberOrTag>,
    ) -> RpcResult<U256> {
        let block_id = self.resolve_block_id(block.unwrap_or(BlockNumberOrTag::Latest))?;

        let gas = self.estimate_gas
            .execute(call, block_id)
            .await
            .map_err(|e| RpcError::internal(e.to_string()))?;

        Ok(U256::from(gas))
    }
}
```

---

## 6. 性能优化指南

### 6.1 缓存策略

```rust
// 多级缓存架构
pub struct CachedStateDatabase {
    // L1: 内存 LRU 缓存
    l1_cache: Arc<Mutex<LruCache<H256, Account>>>,
    // L2: Redis 缓存
    l2_cache: Arc<RedisClient>,
    // L3: 数据库
    db: Arc<dyn KeyValueStore>,
}

impl CachedStateDatabase {
    pub async fn get_account(&self, address: Address) -> Result<Option<Account>, Error> {
        let key = keccak256(address.as_bytes());

        // 1. 检查 L1 缓存
        {
            let mut l1 = self.l1_cache.lock().await;
            if let Some(account) = l1.get(&key) {
                return Ok(Some(account.clone()));
            }
        }

        // 2. 检查 L2 缓存
        if let Some(data) = self.l2_cache.get(&key).await? {
            let account: Account = bincode::deserialize(&data)?;

            // 回填 L1
            let mut l1 = self.l1_cache.lock().await;
            l1.put(key, account.clone());

            return Ok(Some(account));
        }

        // 3. 从数据库读取
        let data = self.db.get(key.as_slice()).await?;
        if let Some(data) = data {
            let account: Account = rlp::decode(&data)?;

            // 回填缓存
            let serialized = bincode::serialize(&account)?;
            self.l2_cache.set(&key, &serialized, 3600).await?;

            let mut l1 = self.l1_cache.lock().await;
            l1.put(key, account.clone());

            return Ok(Some(account));
        }

        Ok(None)
    }
}
```

---

### 6.2 批量操作优化

```rust
// 批量获取余额
impl EthApiHandler {
    pub async fn get_balance_batch(
        &self,
        addresses: Vec<Address>,
        block_id: BlockId,
    ) -> RpcResult<Vec<U256>> {
        let block_hash = self.resolve_block_id_to_hash(block_id).await?;
        let state = self.state_db.at_block(block_hash).await?;

        // 并行查询所有地址
        let futures = addresses.into_iter().map(|addr| {
            let state = state.clone();
            async move {
                state.get_account(addr)
                    .await
                    .map(|acc| acc.map(|a| a.balance).unwrap_or(U256::ZERO))
            }
        });

        let results = futures::future::join_all(futures).await;

        results.into_iter().collect::<Result<Vec<_>, _>>()
            .map_err(|e| RpcError::internal(e.to_string()))
    }
}
```

---

### 6.3 连接池和资源管理

```rust
// 数据库连接池
pub struct RocksDBPool {
    connections: Arc<Vec<Arc<DB>>>,
    next_idx: AtomicUsize,
}

impl RocksDBPool {
    pub fn new(path: &str, pool_size: usize) -> Result<Self, Error> {
        let mut connections = Vec::new();

        for _ in 0..pool_size {
            let db = Arc::new(DB::open_default(path)?);
            connections.push(db);
        }

        Ok(Self {
            connections: Arc::new(connections),
            next_idx: AtomicUsize::new(0),
        })
    }

    pub fn get(&self) -> Arc<DB> {
        let idx = self.next_idx.fetch_add(1, Ordering::Relaxed) % self.connections.len();
        self.connections[idx].clone()
    }
}
```

---

### 6.4 零分配路径

```rust
// 使用栈分配避免堆分配
pub fn encode_block_number_fast(number: u64) -> [u8; 10] {
    let mut buf = [0u8; 10];
    buf[0] = b'0';
    buf[1] = b'x';

    // 手动转换为十六进制
    let mut n = number;
    let mut i = 9;
    while n > 0 && i >= 2 {
        let digit = (n % 16) as u8;
        buf[i] = if digit < 10 {
            b'0' + digit
        } else {
            b'a' + (digit - 10)
        };
        n /= 16;
        i -= 1;
    }

    buf
}

// 使用 SmallVec 避免小数组堆分配
use smallvec::SmallVec;

pub fn collect_transaction_hashes(block: &Block) -> SmallVec<[H256; 128]> {
    let mut hashes = SmallVec::new();
    for tx in &block.transactions {
        hashes.push(tx.hash());
    }
    hashes
}
```

---

### 6.5 CPU 绑定和优先级

```rust
// 将 RPC 线程绑定到特定 CPU 核心
use core_affinity;

pub fn bind_rpc_threads() {
    let core_ids = core_affinity::get_core_ids().unwrap();

    // 绑定到后半部分 CPU 核心
    let rpc_cores = &core_ids[core_ids.len() / 2..];

    for (i, core_id) in rpc_cores.iter().enumerate() {
        std::thread::Builder::new()
            .name(format!("rpc-worker-{}", i))
            .spawn(move || {
                core_affinity::set_for_current(*core_id);

                // RPC 处理逻辑
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        // ...
                    });
            })
            .unwrap();
    }
}
```

---

## 7. 错误处理规范

### 7.1 标准错误码

基于 JSON-RPC 2.0 规范：

| 错误码 | 消息 | 含义 |
|--------|------|------|
| `-32700` | Parse error | 无效的 JSON |
| `-32600` | Invalid Request | JSON-RPC 请求无效 |
| `-32601` | Method not found | 方法不存在 |
| `-32602` | Invalid params | 无效的参数 |
| `-32603` | Internal error | 内部错误 |
| `-32000` | Server error | 服务器错误 |
| `-32001` | Nonce too low | Nonce 太低 |
| `-32002` | Insufficient funds | 余额不足 |
| `-32003` | Gas limit exceeded | Gas 限制超出 |
| `-32004` | Transaction underpriced | 交易价格过低 |
| `-32005` | Limit exceeded | 请求限制超出 |

---

### 7.2 错误实现

```rust
// interfaces/rpc/error.rs
use jsonrpsee::types::ErrorObjectOwned;

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("Method not found")]
    MethodNotFound,

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Nonce too low")]
    NonceTooLow,

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Gas limit exceeded")]
    GasLimitExceeded,

    #[error("Execution reverted: {0:?}")]
    ExecutionReverted(Bytes),

    #[error("Block not found")]
    BlockNotFound,

    #[error("Transaction not found")]
    TransactionNotFound,

    #[error("Request limit exceeded")]
    LimitExceeded,
}

impl From<RpcError> for ErrorObjectOwned {
    fn from(err: RpcError) -> Self {
        match err {
            RpcError::MethodNotFound => {
                ErrorObjectOwned::owned(-32601, "Method not found", None::<()>)
            }
            RpcError::InvalidParams(msg) => {
                ErrorObjectOwned::owned(-32602, msg, None::<()>)
            }
            RpcError::InternalError(msg) => {
                ErrorObjectOwned::owned(-32603, format!("Internal error: {}", msg), None::<()>)
            }
            RpcError::NonceTooLow => {
                ErrorObjectOwned::owned(-32001, "Nonce too low", None::<()>)
            }
            RpcError::InsufficientFunds => {
                ErrorObjectOwned::owned(-32002, "Insufficient funds", None::<()>)
            }
            RpcError::GasLimitExceeded => {
                ErrorObjectOwned::owned(-32003, "Gas limit exceeded", None::<()>)
            }
            RpcError::ExecutionReverted(data) => {
                ErrorObjectOwned::owned(-32000, "Execution reverted", Some(data))
            }
            RpcError::BlockNotFound => {
                ErrorObjectOwned::owned(-32000, "Block not found", None::<()>)
            }
            RpcError::TransactionNotFound => {
                ErrorObjectOwned::owned(-32000, "Transaction not found", None::<()>)
            }
            RpcError::LimitExceeded => {
                ErrorObjectOwned::owned(-32005, "Request limit exceeded", None::<()>)
            }
        }
    }
}
```

---

## 总结

本文档详细描述了基于以太坊官方标准的执行客户端 JSON-RPC API 规范，包括：

1. **标准 JSON-RPC API**: 完整的 `eth_*` 命名空间方法实现
2. **Engine API**: 共识客户端与执行客户端之间的通信接口
3. **Admin & Debug API**: 节点管理和调试工具
4. **Clean Architecture 实现**: 基于领域驱动设计的分层架构
5. **性能优化指南**: 低延迟、高吞吐量的实现策略
6. **错误处理规范**: 标准化的错误码和错误处理机制

所有 API 实现遵循：
- **以太坊官方规范**: 基于 `ethereum/execution-apis` 标准
- **Clean Architecture**: 领域层、应用层、接口层、基础设施层清晰分离
- **低延迟优化**: 缓存、批量操作、零分配路径、CPU 绑定
- **类型安全**: 使用强类型系统避免运行时错误
- **可测试性**: 通过依赖注入和接口抽象实现高可测试性

---

**参考资料**:
- [Ethereum JSON-RPC Specification](https://ethereum.github.io/execution-apis/api-documentation/)
- [EIP-1474: Remote procedure call specification](https://eips.ethereum.org/EIPS/eip-1474)
- [Engine API Specification](https://github.com/ethereum/execution-apis/tree/main/src/engine)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)

**文档版本**: v1.0.0
**最后更新**: 2025-10-16
