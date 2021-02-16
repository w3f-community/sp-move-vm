mod common;

use crate::common::{ident, StorageMock, DSMock, module_id, dfi_module, account_module, call_module, balance_test_module, signer_module};
use common::{addr, BankMock};
use move_core_types::language_storage::{StructTag, TypeTag, CORE_CODE_ADDRESS, ModuleId};
use move_vm_runtime::loader::Loader;
use mvm::storage::bank::{Bank, ACCOUNT_MODULE, BALANCE_STRUCT, DFI_MODULE, TypeWalker, BalanceHandler};
use mvm::storage::session::{Resolve, ResolverResult};
use mvm::storage::store::DataAccess;
use move_vm_runtime::logging::NoContextLog;
use move_vm_types::data_store::DataStore;
use std::rc::Rc;

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

    assert_eq!(
        ResolverResult::Resolved(Ok(Some(usd.to_le_bytes().to_vec()))),
        session.resolve(&addr("0x022"), &balance("USD"))
    );
    assert_eq!(
        ResolverResult::Resolved(Ok(None)),
        session.resolve(&addr("0x021"), &balance("USD"))
    );
    assert_eq!(
        ResolverResult::Resolved(Ok(None)),
        session.resolve(&addr("0x022"), &balance("BTC"))
    );
    assert_eq!(
        ResolverResult::Unresolved,
        session.resolve(
            &addr("0x022"),
            &StructTag {
                address: CORE_CODE_ADDRESS,
                module: ACCOUNT_MODULE.to_owned(),
                name: BALANCE_STRUCT.to_owned(),
                type_params: vec![TypeTag::U8],
            },
        )
    );
}

#[test]
fn test_balance_handler() {
    let loader = Loader::new();
    let walker = TypeWalker::new(&loader);
    let mut data = DSMock::default();
    let mut log = NoContextLog::new();

    data.publish_module(&module_id("0x01", "Signer"), signer_module().code().to_vec()).unwrap();
    data.publish_module(&module_id("0x01", "Dfinance"), dfi_module().code().to_vec()).unwrap();
    data.publish_module(&module_id("0x01", "Account"), account_module().code().to_vec()).unwrap();
    data.publish_module(&module_id("0x01", "Call"), call_module().code().to_vec()).unwrap();
    data.publish_module(&module_id("0x01", "BalanceTest"), balance_test_module().code().to_vec()).unwrap();

    // let lock1 = StructTag {
    //     address: CORE_CODE_ADDRESS,
    //     module: ident("BalanceTest"),
    //     name: ident("Lock1"),
    //     type_params: vec![
    //         TypeTag::Struct(StructTag {
    //             address: CORE_CODE_ADDRESS,
    //             module: ident("Dfinance"),
    //             name: ident("USD"),
    //             type_params: vec![],
    //         })
    //     ],
    // };
    //
    // let tp = loader.load_type(&TypeTag::Struct(lock1.clone()), &mut data, &mut log).unwrap();
    // assert_eq!(
    //     BalanceHandler::Locked(vec![(Rc::new("USD".to_owned()), vec![0, 0])]),
    //     walker.find_balance(&lock1, &tp).unwrap().unwrap()
    // );
    //
    // let lock2 = StructTag {
    //     address: CORE_CODE_ADDRESS,
    //     module: ident("BalanceTest"),
    //     name: ident("Lock2"),
    //     type_params: vec![]
    // };
    // let tp = loader.load_type(&TypeTag::Struct(lock2.clone()), &mut data, &mut log).unwrap();
    // assert_eq!(
    //     BalanceHandler::Locked(vec![(Rc::new("BTC".to_owned()), vec![0, 0])]),
    //     walker.find_balance(&lock2, &tp).unwrap().unwrap()
    // );

    let lock3 = StructTag {
        address: CORE_CODE_ADDRESS,
        module: ident("BalanceTest"),
        name: ident("Lock3"),
        type_params: vec![
            TypeTag::Struct(StructTag {
                address: CORE_CODE_ADDRESS,
                module: ident("Dfinance"),
                name: ident("USD"),
                type_params: vec![],
            })
        ],
    };

    let tp = loader.load_type(&TypeTag::Struct(lock3.clone()), &mut data, &mut log).unwrap();
    assert_eq!(
        BalanceHandler::Locked(vec![(Rc::new("USD".to_owned()), vec![0, 0])]),
        walker.find_balance(&lock3, &tp).unwrap().unwrap()
    );
}
