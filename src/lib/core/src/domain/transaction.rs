// Ethereum Transaction Domain Model
//
// Following Clean Architecture principles - Domain layer has no external dependencies
// Reference: /Users/hongyaotang/src/reth-ddd/design/trans-flow.md

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ============================================================================
// Basic Data Types
// ============================================================================

/// Ethereum Address (20 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn zero() -> Self {
        Address([0u8; 20])
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, DomainError> {
        if slice.len() != 20 {
            return Err(DomainError::InvalidAddress);
        }
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(slice);
        Ok(Address(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// 256-bit Hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct H256(pub [u8; 32]);

impl H256 {
    pub fn zero() -> Self {
        H256([0u8; 32])
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, DomainError> {
        if slice.len() != 32 {
            return Err(DomainError::InvalidHash);
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(slice);
        Ok(H256(bytes))
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// 256-bit Unsigned Integer (simplified, should use proper U256 library)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct U256(pub u128);

impl U256 {
    pub const ZERO: U256 = U256(0);

    pub fn from_u64(value: u64) -> Self {
        U256(value as u128)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn checked_add(self, other: U256) -> Option<U256> {
        self.0.checked_add(other.0).map(U256)
    }

    pub fn checked_sub(self, other: U256) -> Option<U256> {
        self.0.checked_sub(other.0).map(U256)
    }

    pub fn saturating_sub(self, other: U256) -> U256 {
        U256(self.0.saturating_sub(other.0))
    }
}

/// Bytes array
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    pub fn new() -> Self {
        Bytes(Vec::new())
    }

    pub fn from_vec(vec: Vec<u8>) -> Self {
        Bytes(vec)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// ============================================================================
// Transaction Types
// ============================================================================

/// Transaction Type Enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TransactionType {
    /// Legacy transaction (pre-EIP-2718)
    Legacy = 0x00,
    /// EIP-2930: Access list transaction
    Eip2930 = 0x01,
    /// EIP-1559: Fee market transaction
    Eip1559 = 0x02,
    /// EIP-4844: Blob transaction
    Eip4844 = 0x03,
}

impl TransactionType {
    pub fn from_u8(value: u8) -> Result<Self, DomainError> {
        match value {
            0x00 => Ok(TransactionType::Legacy),
            0x01 => Ok(TransactionType::Eip2930),
            0x02 => Ok(TransactionType::Eip1559),
            0x03 => Ok(TransactionType::Eip4844),
            _ => Err(DomainError::InvalidTransactionType),
        }
    }
}

/// Main Transaction Enum - supports multiple transaction types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Transaction {
    /// Legacy transaction
    Legacy(LegacyTransaction),
    /// EIP-2930 transaction
    Eip2930(Eip2930Transaction),
    /// EIP-1559 transaction (current mainstream)
    Eip1559(Eip1559Transaction),
    /// EIP-4844 Blob transaction
    Eip4844(Eip4844Transaction),
}

// ============================================================================
// Legacy Transaction (pre-EIP-1559)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyTransaction {
    /// Chain ID
    pub chain_id: u64,
    /// Sender's transaction sequence number (prevents replay attacks)
    pub nonce: u64,
    /// Gas price (Wei per gas)
    pub gas_price: U256,
    /// Gas limit
    pub gas_limit: u64,
    /// Recipient address (None means contract creation)
    pub to: Option<Address>,
    /// Transfer amount (Wei)
    pub value: U256,
    /// Transaction data (contract call data or creation code)
    pub data: Bytes,
    /// Signature v value
    pub v: u64,
    /// Signature r value
    pub r: H256,
    /// Signature s value
    pub s: H256,
}

// ============================================================================
// EIP-2930 Transaction (Access List)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eip2930Transaction {
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Bytes,
    /// Access list (pre-declares accessed addresses and storage slots)
    pub access_list: Vec<AccessListItem>,
    pub v: u64,
    pub r: H256,
    pub s: H256,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

// ============================================================================
// EIP-1559 Transaction (Fee Market) - Current Mainstream
// ============================================================================

/// EIP-1559 Transaction Structure
///
/// Features:
/// - Dynamic base fee (automatically adjusted by protocol)
/// - maxPriorityFeePerGas (tip for validators)
/// - maxFeePerGas (maximum willing to pay)
///
/// Actual fee calculation:
/// effectiveGasPrice = baseFee + min(maxPriorityFeePerGas, maxFeePerGas - baseFee)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eip1559Transaction {
    /// Chain ID (prevents cross-chain replay attacks)
    pub chain_id: u64,

    /// Nonce - sender account's transaction counter
    /// Must equal current account nonce, ensures sequential execution
    pub nonce: u64,

    /// Maximum priority fee (tip for validators, Wei per gas)
    /// Typical value: 1-3 Gwei
    pub max_priority_fee_per_gas: U256,

    /// Maximum total fee (Wei per gas)
    /// Must be >= baseFeePerGas + maxPriorityFeePerGas
    /// Typical value: 30-100 Gwei
    pub max_fee_per_gas: U256,

    /// Gas limit
    /// Simple transfer: 21,000
    /// Contract call: depends on complexity
    pub gas_limit: u64,

    /// Recipient address
    /// None means contract creation transaction
    pub to: Option<Address>,

    /// Transfer amount (Wei)
    /// 1 ETH = 10^18 Wei
    pub value: U256,

    /// Transaction data
    /// Simple transfer: empty (0x)
    /// Contract call: function selector + parameters
    /// Contract creation: contract bytecode
    pub data: Bytes,

    /// Access list (EIP-2930)
    /// Pre-declares addresses and storage slots to be accessed, can save gas
    pub access_list: Vec<AccessListItem>,

    // === Signature fields ===

    /// Recovery ID (EIP-155: chainId * 2 + 35 + {0,1})
    pub v: u64,

    /// ECDSA signature r value
    pub r: H256,

    /// ECDSA signature s value
    pub s: H256,
}

// ============================================================================
// EIP-4844 Transaction (Blob Transaction)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Eip4844Transaction {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub gas_limit: u64,
    pub to: Address, // Blob transactions must have recipient
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListItem>,
    /// Versioned hashes of blob data
    pub blob_versioned_hashes: Vec<H256>,
    /// Maximum blob gas fee
    pub max_fee_per_blob_gas: U256,
    pub v: u64,
    pub r: H256,
    pub s: H256,
}

// ============================================================================
// Transaction Implementation - Unified Interface
// ============================================================================

impl Transaction {
    /// Get transaction type
    pub fn transaction_type(&self) -> TransactionType {
        match self {
            Transaction::Legacy(_) => TransactionType::Legacy,
            Transaction::Eip2930(_) => TransactionType::Eip2930,
            Transaction::Eip1559(_) => TransactionType::Eip1559,
            Transaction::Eip4844(_) => TransactionType::Eip4844,
        }
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        match self {
            Transaction::Legacy(tx) => tx.chain_id,
            Transaction::Eip2930(tx) => tx.chain_id,
            Transaction::Eip1559(tx) => tx.chain_id,
            Transaction::Eip4844(tx) => tx.chain_id,
        }
    }

    /// Get nonce
    pub fn nonce(&self) -> u64 {
        match self {
            Transaction::Legacy(tx) => tx.nonce,
            Transaction::Eip2930(tx) => tx.nonce,
            Transaction::Eip1559(tx) => tx.nonce,
            Transaction::Eip4844(tx) => tx.nonce,
        }
    }

    /// Get gas limit
    pub fn gas_limit(&self) -> u64 {
        match self {
            Transaction::Legacy(tx) => tx.gas_limit,
            Transaction::Eip2930(tx) => tx.gas_limit,
            Transaction::Eip1559(tx) => tx.gas_limit,
            Transaction::Eip4844(tx) => tx.gas_limit,
        }
    }

    /// Get recipient address
    pub fn to(&self) -> Option<Address> {
        match self {
            Transaction::Legacy(tx) => tx.to,
            Transaction::Eip2930(tx) => tx.to,
            Transaction::Eip1559(tx) => tx.to,
            Transaction::Eip4844(tx) => Some(tx.to),
        }
    }

    /// Get transfer amount
    pub fn value(&self) -> U256 {
        match self {
            Transaction::Legacy(tx) => tx.value,
            Transaction::Eip2930(tx) => tx.value,
            Transaction::Eip1559(tx) => tx.value,
            Transaction::Eip4844(tx) => tx.value,
        }
    }

    /// Get transaction data
    pub fn data(&self) -> &Bytes {
        match self {
            Transaction::Legacy(tx) => &tx.data,
            Transaction::Eip2930(tx) => &tx.data,
            Transaction::Eip1559(tx) => &tx.data,
            Transaction::Eip4844(tx) => &tx.data,
        }
    }

    /// Compute transaction hash (Keccak256)
    ///
    /// Note: Should use Keccak256, using SHA256 for simplified demo
    pub fn compute_hash(&self) -> H256 {
        let encoded = self.encode_for_signing();
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        let result = hasher.finalize();

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        H256(hash)
    }

    /// Encode transaction for signing
    ///
    /// Format: RLP([chainId, nonce, maxPriorityFee, maxFee, gasLimit, to, value, data, accessList])
    fn encode_for_signing(&self) -> Vec<u8> {
        // Simplified implementation: should use RLP encoding
        match self {
            Transaction::Eip1559(tx) => {
                format!(
                    "{}:{}:{}:{}:{}:{:?}:{}:{}",
                    tx.chain_id,
                    tx.nonce,
                    tx.max_priority_fee_per_gas.0,
                    tx.max_fee_per_gas.0,
                    tx.gas_limit,
                    tx.to,
                    tx.value.0,
                    tx.data.len()
                ).into_bytes()
            }
            _ => Vec::new(),
        }
    }

    /// Recover sender address
    ///
    /// Recover public key from signature (v, r, s), then derive address from public key
    ///
    /// Note: Actual implementation needs secp256k1 library
    pub fn recover_sender(&self) -> Result<Address, DomainError> {
        // Simplified implementation: should use ECDSA recovery
        // Returning placeholder address
        Ok(Address::zero())
    }
}

// ============================================================================
// EIP-1559 Transaction Implementation - Business Rules
// ============================================================================

impl Eip1559Transaction {
    /// Create new EIP-1559 transaction
    pub fn new(
        chain_id: u64,
        nonce: u64,
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
        gas_limit: u64,
        to: Option<Address>,
        value: U256,
        data: Bytes,
    ) -> Self {
        Self {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            to,
            value,
            data,
            access_list: Vec::new(),
            v: 0,
            r: H256::zero(),
            s: H256::zero(),
        }
    }

    /// Validate basic transaction validity
    ///
    /// Includes:
    /// - Gas limit check
    /// - Fee parameter check
    /// - Data size check
    pub fn validate(&self) -> Result<(), DomainError> {
        // === Validation 1: Gas Limit ===
        if self.gas_limit < 21_000 {
            return Err(DomainError::GasLimitTooLow);
        }

        if self.gas_limit > 30_000_000 {
            return Err(DomainError::GasLimitTooHigh);
        }

        // === Validation 2: Fee Parameters ===
        if self.max_fee_per_gas < U256::from_u64(1_000_000_000) {
            // Minimum 1 Gwei
            return Err(DomainError::GasPriceTooLow);
        }

        if self.max_priority_fee_per_gas > self.max_fee_per_gas {
            return Err(DomainError::PriorityFeeExceedsMaxFee);
        }

        // === Validation 3: Data Size ===
        if self.data.len() > 128 * 1024 {
            // Maximum 128KB
            return Err(DomainError::DataTooLarge);
        }

        Ok(())
    }

    /// Calculate actual paid gas price
    ///
    /// effectiveGasPrice = baseFee + min(maxPriorityFee, maxFee - baseFee)
    pub fn effective_gas_price(&self, base_fee: U256) -> U256 {
        let max_priority_fee = self.max_priority_fee_per_gas;
        let max_fee_minus_base = self.max_fee_per_gas.saturating_sub(base_fee);

        let priority_fee = if max_priority_fee.0 < max_fee_minus_base.0 {
            max_priority_fee
        } else {
            max_fee_minus_base
        };

        base_fee.checked_add(priority_fee).unwrap_or(base_fee)
    }

    /// Calculate maximum possible cost
    ///
    /// maxCost = value + (gasLimit * maxFeePerGas)
    pub fn max_cost(&self) -> Option<U256> {
        let gas_cost = U256::from_u64(self.gas_limit)
            .checked_add(self.max_fee_per_gas)?;

        self.value.checked_add(gas_cost)
    }

    /// Check if transaction is a simple transfer
    pub fn is_simple_transfer(&self) -> bool {
        self.to.is_some() && self.data.is_empty()
    }

    /// Check if transaction is contract creation
    pub fn is_contract_creation(&self) -> bool {
        self.to.is_none()
    }
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// Invalid address format
    InvalidAddress,
    /// Invalid hash format
    InvalidHash,
    /// Invalid transaction type
    InvalidTransactionType,
    /// Gas limit too low
    GasLimitTooLow,
    /// Gas limit too high
    GasLimitTooHigh,
    /// Gas price too low
    GasPriceTooLow,
    /// Priority fee exceeds maximum fee
    PriorityFeeExceedsMaxFee,
    /// Data too large
    DataTooLarge,
    /// Invalid signature
    InvalidSignature,
    /// Nonce too low
    NonceTooLow,
    /// Insufficient funds
    InsufficientFunds,
    /// Invalid chain ID
    InvalidChainId,
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidAddress => write!(f, "Invalid address format"),
            DomainError::InvalidHash => write!(f, "Invalid hash format"),
            DomainError::InvalidTransactionType => write!(f, "Invalid transaction type"),
            DomainError::GasLimitTooLow => write!(f, "Gas limit too low (minimum 21,000)"),
            DomainError::GasLimitTooHigh => write!(f, "Gas limit too high (maximum 30,000,000)"),
            DomainError::GasPriceTooLow => write!(f, "Gas price too low (minimum 1 Gwei)"),
            DomainError::PriorityFeeExceedsMaxFee => {
                write!(f, "Priority fee exceeds maximum fee")
            }
            DomainError::DataTooLarge => write!(f, "Transaction data too large (max 128KB)"),
            DomainError::InvalidSignature => write!(f, "Invalid signature"),
            DomainError::NonceTooLow => write!(f, "Nonce too low"),
            DomainError::InsufficientFunds => write!(f, "Insufficient funds"),
            DomainError::InvalidChainId => write!(f, "Invalid chain ID"),
        }
    }
}

impl std::error::Error for DomainError {}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_simple_transfer() {
        // Alice -> Bob transfer 1 ETH
        let tx = Eip1559Transaction::new(
            1,                                    // Mainnet
            42,                                   // nonce
            U256::from_u64(2_000_000_000),       // 2 Gwei priority fee
            U256::from_u64(50_000_000_000),      // 50 Gwei max fee
            21_000,                               // standard transfer gas
            Some(Address::zero()),                // Bob's address
            U256::from_u64(1_000_000_000_000_000_000), // 1 ETH
            Bytes::new(),                         // empty data
        );

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.nonce, 42);
        assert_eq!(tx.gas_limit, 21_000);
        assert!(tx.is_simple_transfer());
        assert!(!tx.is_contract_creation());
    }

    #[test]
    fn test_transaction_validation() {
        let tx = Eip1559Transaction::new(
            1,
            0,
            U256::from_u64(2_000_000_000),
            U256::from_u64(50_000_000_000),
            21_000,
            Some(Address::zero()),
            U256::ZERO,
            Bytes::new(),
        );

        assert!(tx.validate().is_ok());
    }

    #[test]
    fn test_gas_limit_too_low() {
        let tx = Eip1559Transaction::new(
            1,
            0,
            U256::from_u64(2_000_000_000),
            U256::from_u64(50_000_000_000),
            20_000, // Too low!
            Some(Address::zero()),
            U256::ZERO,
            Bytes::new(),
        );

        assert_eq!(tx.validate(), Err(DomainError::GasLimitTooLow));
    }

    #[test]
    fn test_priority_fee_exceeds_max_fee() {
        let tx = Eip1559Transaction::new(
            1,
            0,
            U256::from_u64(60_000_000_000), // Higher than max fee!
            U256::from_u64(50_000_000_000),
            21_000,
            Some(Address::zero()),
            U256::ZERO,
            Bytes::new(),
        );

        assert_eq!(tx.validate(), Err(DomainError::PriorityFeeExceedsMaxFee));
    }

    #[test]
    fn test_effective_gas_price() {
        let tx = Eip1559Transaction::new(
            1,
            0,
            U256::from_u64(2_000_000_000),  // 2 Gwei priority
            U256::from_u64(50_000_000_000), // 50 Gwei max
            21_000,
            Some(Address::zero()),
            U256::ZERO,
            Bytes::new(),
        );

        // Base fee = 48 Gwei
        let base_fee = U256::from_u64(48_000_000_000);

        // Effective = 48 + min(2, 50-48) = 48 + 2 = 50 Gwei
        let effective = tx.effective_gas_price(base_fee);
        assert_eq!(effective, U256::from_u64(50_000_000_000));
    }

    #[test]
    fn test_max_cost_calculation() {
        let tx = Eip1559Transaction::new(
            1,
            0,
            U256::from_u64(2_000_000_000),
            U256::from_u64(50_000_000_000),
            21_000,
            Some(Address::zero()),
            U256::from_u64(1_000_000_000_000_000_000), // 1 ETH
            Bytes::new(),
        );

        let max_cost = tx.max_cost().unwrap();

        // Max cost = 1 ETH + (21000 * 50 Gwei)
        // = 1_000_000_000_000_000_000 + 1_050_000_000_000_000
        // = 1_001_050_000_000_000_000 Wei
        assert!(max_cost.0 > 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_transaction_type() {
        let tx = Transaction::Eip1559(Eip1559Transaction::new(
            1, 0,
            U256::from_u64(2_000_000_000),
            U256::from_u64(50_000_000_000),
            21_000,
            Some(Address::zero()),
            U256::ZERO,
            Bytes::new(),
        ));

        assert_eq!(tx.transaction_type(), TransactionType::Eip1559);
        assert_eq!(tx.chain_id(), 1);
        assert_eq!(tx.nonce(), 0);
        assert_eq!(tx.gas_limit(), 21_000);
    }
}
