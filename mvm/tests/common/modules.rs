use mvm::types::ModuleTx;
use move_core_types::language_storage::CORE_CODE_ADDRESS;

pub fn store_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/2_Store.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn event_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/0_Event.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn vector_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/3_Vector.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn signer_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/1_Signer.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn dfi_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/4_Dfinance.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn account_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/5_Account.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn call_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/6_Call.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}

pub fn balance_test_module() -> ModuleTx {
    ModuleTx::new(
        include_bytes!("../assets/target/modules/7_BalanceTest.mv").to_vec(),
        CORE_CODE_ADDRESS,
    )
}
