/// 领域层模块
///
/// 遵循 Clean Architecture 原则
/// 领域层包含纯业务逻辑，不依赖任何外部框架

pub mod transaction;

// 重新导出常用类型
pub use transaction::{
    Address, Bytes, DomainError, Eip1559Transaction, H256, Transaction, TransactionType, U256,
};
