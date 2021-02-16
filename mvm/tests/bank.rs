mod common;

use mvm::storage::bank::{Bank, ACCOUNT_MODULE, BALANCE_STRUCT, DFI_MODULE};
use common::{
    BankMock, addr,
};
use move_vm_runtime::loader::Loader;
use mvm::storage::session::{Resolve, ResolverResult};
use move_core_types::language_storage::{StructTag, CORE_CODE_ADDRESS, TypeTag};
use crate::common::ident;

fn balance(ticker: &str) -> StructTag {
    StructTag {
        address: CORE_CODE_ADDRESS,
        module: ACCOUNT_MODULE.to_owned(),
        name: BALANCE_STRUCT.to_owned(),
        type_params: vec![TypeTag::Struct(StructTag {
            address: CORE_CODE_ADDRESS,
            module: DFI_MODULE.to_owned(),
            name: ident(ticker),
            type_params: vec![],
        })],
    }
}

#[test]
fn test_load_balance() {
    let usd = 1313;
    let bank = Bank::new(BankMock::with_data(&[("USD", "0x022", usd, true)]));
    let loader = Loader::new();
    let session = bank.new_session(&loader);

    assert_eq!(ResolverResult::Resolved(Ok(Some(usd.to_le_bytes().to_vec()))),
               session.resolve(&addr("0x022"), &balance("USD"))
    );
    assert_eq!(ResolverResult::Resolved(Ok(None)),
               session.resolve(&addr("0x021"), &balance("USD"))
    );
    assert_eq!(ResolverResult::Resolved(Ok(None)),
               session.resolve(&addr("0x022"), &balance("BTC"))
    );
    assert_eq!(ResolverResult::Unresolved,
               session.resolve(&addr("0x022"), &StructTag {
                   address: CORE_CODE_ADDRESS,
                   module: ACCOUNT_MODULE.to_owned(),
                   name: BALANCE_STRUCT.to_owned(),
                   type_params: vec![TypeTag::U8],
               })
    );
}