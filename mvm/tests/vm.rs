#[macro_use]
extern crate alloc;

mod common;

use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag, CORE_CODE_ADDRESS};
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::data_cache::RemoteCache;
use mvm::mvm::Mvm;
use mvm::types::{Gas, ModuleTx, ScriptArg, ScriptTx};
use mvm::Vm;
use serde::Deserialize;

use crate::common::BankMock;
use common::{EventHandlerMock, StorageMock};
use mvm::storage::bank::Balances;
use mvm::storage::chain::TxInfo;
use mvm::storage::event::EventHandler;
use mvm::storage::store::RawData;

fn gas() -> Gas {
    Gas::new(10_000, 1).unwrap()
}

fn store_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/2_Store.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn event_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/0_Event.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn vector_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/3_Vector.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn signer_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/1_Signer.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn dfi_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/4_Dfinance.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn account_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/5_Account.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn call_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("assets/target/modules/6_Call.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

fn store_script(args: u64) -> ScriptTx {
    ScriptTx::new(
        include_bytes!("assets/target/scripts/3_store_u64.mv").to_vec(),
        vec![ScriptArg::U64(args)],
        vec![],
        vec![CORE_CODE_ADDRESS],
        TxInfo {
            timestamp: 0,
            block_height: 0,
        },
    )
    .unwrap()
}

fn emit_event_script(args: u64) -> ScriptTx {
    ScriptTx::new(
        include_bytes!("assets/target/scripts/0_emit_event.mv").to_vec(),
        vec![ScriptArg::U64(args)],
        vec![],
        vec![CORE_CODE_ADDRESS],
        TxInfo::default(),
    )
    .unwrap()
}

fn publish_module<S, E, B>(vm: &Mvm<S, E, B>, module: ModuleTx)
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    assert_eq!(
        StatusCode::EXECUTED,
        vm.publish_module(gas(), module).status_code
    );
}

#[derive(Deserialize)]
struct StoreU64 {
    pub val: u64,
}

#[test]
fn test_public_module() {
    let store = StorageMock::new();
    let bank = BankMock::default();
    let vm = Mvm::new(store.clone(), EventHandlerMock::default(), bank.clone()).unwrap();
    let state_session = vm.session(None);

    publish_module(&vm, store_module());

    let store_module_id = ModuleId::new(CORE_CODE_ADDRESS, Identifier::new("Store").unwrap());
    assert_eq!(
        state_session.get_module(&store_module_id).unwrap().unwrap(),
        include_bytes!("assets/target/modules/2_Store.mv").to_vec()
    );
}

#[test]
fn test_execute_script() {
    let test_value = 13;
    let store = StorageMock::new();
    let bank = BankMock::default();
    let event_handler = EventHandlerMock::default();

    let vm = Mvm::new(store.clone(), event_handler.clone(), bank.clone()).unwrap();
    let state_session = vm.session(None);
    publish_module(&vm, store_module());

    assert_eq!(
        StatusCode::EXECUTED,
        vm.execute_script(gas(), store_script(test_value))
            .status_code
    );

    let tag = StructTag {
        address: CORE_CODE_ADDRESS,
        module: Identifier::new("Store").unwrap(),
        name: Identifier::new("U64").unwrap(),
        type_params: vec![],
    };
    let blob = state_session
        .get_resource(&CORE_CODE_ADDRESS, &tag)
        .unwrap()
        .unwrap();
    let store: StoreU64 = bcs::from_bytes(&blob).unwrap();
    assert_eq!(test_value, store.val);
}

#[test]
fn test_store_event() {
    let test_value = 13;
    let mock = StorageMock::new();
    let bank = BankMock::default();

    let event_handler = EventHandlerMock::default();
    let vm = Mvm::new(mock, event_handler.clone(), bank).unwrap();
    publish_module(&vm, vector_module());
    publish_module(&vm, event_module());

    assert_eq!(
        StatusCode::EXECUTED,
        vm.execute_script(gas(), emit_event_script(test_value))
            .status_code
    );

    let (guid, seq, tag, msg) = event_handler.data.borrow_mut().remove(0);
    assert_eq!(guid, b"GUID".to_vec());
    assert_eq!(seq, 1);
    assert_eq!(test_value, bcs::from_bytes::<StoreU64>(&msg).unwrap().val);
    assert_eq!(
        TypeTag::Struct(StructTag {
            address: CORE_CODE_ADDRESS,
            module: Identifier::new("Event").unwrap(),
            name: Identifier::new("U64").unwrap(),
            type_params: vec![],
        }),
        tag
    );
}

fn load_account_and_store_script() -> ScriptTx {
    ScriptTx::new(
        include_bytes!("assets/target/scripts/2_load_modify_and_store_balance.mv").to_vec(),
        vec![],
        vec![],
        vec![CORE_CODE_ADDRESS],
        TxInfo {
            timestamp: 0,
            block_height: 0,
        },
    )
    .unwrap()
}

#[test]
fn test_balance() {
    let bank = BankMock::with_data(&[("BTC", "0x01", 0, true)]);

    let vm = Mvm::new(
        StorageMock::new(),
        EventHandlerMock::default(),
        bank.clone(),
    )
    .unwrap();
    publish_module(&vm, signer_module());
    publish_module(&vm, dfi_module());
    publish_module(&vm, account_module());
    publish_module(&vm, call_module());

    assert_eq!(
        StatusCode::EXECUTED,
        vm.execute_script(gas(), load_account_and_store_script())
            .status_code
    );
}

// #[test]
// fn test_resource_layout() {
//     let store = StorageMock::new();
//     let bank = BankMock::default();
//     let event_handler = EventHandlerMock::default();
//
//     let vm = Mvm::new(store.clone(), event_handler.clone(), bank.clone()).unwrap();
//     let state_session = vm.session(None);
//
//     assert_eq!(
//         StatusCode::EXECUTED,
//         vm.publish_module(
//             gas(),
//             ModuleTx::new(
//                 include_bytes!("assets/target/modules/4_Dfinance.mv").to_vec(),
//                 CORE_CODE_ADDRESS,
//             )
//         )
//         .status_code
//     );
//
//     assert_eq!(
//         StatusCode::EXECUTED,
//         vm.publish_module(
//             gas(),
//             ModuleTx::new(
//                 include_bytes!("assets/target/modules/5_Account.mv").to_vec(),
//                 CORE_CODE_ADDRESS,
//             )
//         )
//         .status_code
//     );
//
//     assert_eq!(
//         StatusCode::EXECUTED,
//         vm.execute_script(
//             gas(),
//             ScriptTx::new(
//                 include_bytes!("assets/target/scripts/1_store_coin.mv").to_vec(),
//                 vec![],
//                 vec![],
//                 vec![CORE_CODE_ADDRESS],
//                 TxInfo {
//                     timestamp: 0,
//                     block_height: 0
//                 }
//             )
//             .unwrap()
//         )
//         .status_code
//     );
//
//     let tag = StructTag {
//         address: CORE_CODE_ADDRESS,
//         module: Identifier::new("Account").unwrap(),
//         name: Identifier::new("Balance").unwrap(),
//         type_params: vec![TypeTag::Struct(StructTag {
//             address: CORE_CODE_ADDRESS,
//             module: Identifier::new("Balance").unwrap(),
//             name: Identifier::new("BTS").unwrap(),
//             type_params: vec![],
//         })],
//     };
//     let blob = state_session
//         .get_resource(&CORE_CODE_ADDRESS, &tag)
//         .unwrap()
//         .unwrap();
//     let store: U128 = bcs::from_bytes(&blob).unwrap();
//     println!("{}- {:?}", hex::encode(&blob), store);
// }

// #[derive(Deserialize, Debug)]
// struct U128 {
//     val: u128,
// }
