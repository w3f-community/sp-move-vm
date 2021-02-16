use crate::storage::session::{Resolve, ResolverResult};
use core::ops::Deref;
use diem_crypto::Lazy;
use move_core_types::account_address::AccountAddress;
use move_core_types::language_storage::StructTag;
use move_core_types::{identifier::Identifier, language_storage::CORE_CODE_ADDRESS};

pub static TIMESTAMP_MODULE: Lazy<StructTag> = Lazy::new(|| StructTag {
    address: CORE_CODE_ADDRESS,
    module: Identifier::new("Timestamp").unwrap(),
    name: Identifier::new("CurrentTimeMicroseconds").unwrap(),
    type_params: vec![],
});

pub static BLOCK_MODULE: Lazy<StructTag> = Lazy::new(|| StructTag {
    address: CORE_CODE_ADDRESS,
    module: Identifier::new("Block").unwrap(),
    name: Identifier::new("BlockMetadata").unwrap(),
    type_params: vec![],
});

#[derive(Debug, Default)]
pub struct TxInfo {
    pub timestamp: u64,
    pub block_height: u64,
}

impl TxInfo {
    pub fn new(timestamp: u64, block_height: u64) -> TxInfo {
        TxInfo {
            timestamp,
            block_height,
        }
    }
}

pub struct ExecutionContext {
    info: Option<TxInfo>,
}

impl ExecutionContext {
    pub fn new(info: Option<TxInfo>) -> ExecutionContext {
        ExecutionContext { info }
    }
}

impl Resolve for ExecutionContext {
    fn resolve(&self, addr: &AccountAddress, tag: &StructTag) -> ResolverResult {
        if *addr == CORE_CODE_ADDRESS {
            if tag == TIMESTAMP_MODULE.deref() {
                if let Some(info) = &self.info {
                    ResolverResult::Resolved(Ok(Some(info.timestamp.to_le_bytes().to_vec())))
                } else {
                    ResolverResult::Resolved(Ok(None))
                }
            } else if tag == BLOCK_MODULE.deref() {
                if let Some(info) = &self.info {
                    ResolverResult::Resolved(Ok(Some(info.block_height.to_le_bytes().to_vec())))
                } else {
                    ResolverResult::Resolved(Ok(None))
                }
            } else {
                ResolverResult::Unresolved
            }
        } else {
            ResolverResult::Unresolved
        }
    }
}
