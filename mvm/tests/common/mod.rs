use core::cell::RefCell;
use move_core_types::account_address::AccountAddress;
use move_core_types::language_storage::TypeTag;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use mvm::storage::event::EventHandler;
use mvm::storage::store::RawData;
use mvm::storage::bank::{Account, Balances};
use move_core_types::identifier::Identifier;

#[derive(Clone)]
pub struct StorageMock {
    pub data: Rc<RefCell<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl StorageMock {
    pub fn new() -> StorageMock {
        StorageMock {
            data: Rc::new(RefCell::new(Default::default())),
        }
    }
}

impl Default for StorageMock {
    fn default() -> Self {
        StorageMock::new()
    }
}

impl RawData for StorageMock {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let data = self.data.borrow();
        data.get(key).map(|blob| blob.to_owned())
    }

    fn insert(&self, key: &[u8], value: &[u8]) {
        let mut data = self.data.borrow_mut();
        data.insert(key.to_owned(), value.to_owned());
    }

    fn remove(&self, key: &[u8]) {
        let mut data = self.data.borrow_mut();
        data.remove(key);
    }
}

#[derive(Clone, Default)]
pub struct EventHandlerMock {
    pub data: Rc<RefCell<Vec<(Vec<u8>, u64, TypeTag, Vec<u8>)>>>,
}

impl EventHandler for EventHandlerMock {
    fn on_event(&self, guid: Vec<u8>, seq_num: u64, ty_tag: TypeTag, message: Vec<u8>) {
        let mut data = self.data.borrow_mut();
        data.push((guid, seq_num, ty_tag, message));
    }
}

#[derive(Clone, Default)]
pub struct BankMock {
    accounts: Arc<Mutex<HashMap<(String, AccountAddress), Account>>>,
}

impl BankMock {
    pub fn with_data(data: &[(&str, &str, u128, bool)]) -> BankMock {
        let accounts = data
            .iter()
            .map(|(ticker, address, amount, is_lockable)| {
                (
                    (
                        ticker.to_string(),
                        AccountAddress::from_hex_literal(*address).unwrap(),
                    ),
                    Account {
                        amount: *amount,
                        is_lockable: *is_lockable,
                    },
                )
            })
            .collect();
        BankMock {
            accounts: Arc::new(Mutex::new(accounts)),
        }
    }
}

impl Balances for BankMock {
    fn get_balance(&self, ticker: &str, addr: &AccountAddress) -> Option<Account> {
        self.accounts
            .lock()
            .unwrap()
            .get(&(ticker.to_string(), addr.to_owned()))
            .cloned()
    }

    fn transfer(&self, ticker: &str, from: &AccountAddress, to: &AccountAddress, amount: u128) {
        let mut accounts = self.accounts.lock().unwrap();

        let from = accounts
            .get_mut(&(ticker.to_string(), from.to_owned()))
            .unwrap();
        from.amount = from.amount - amount;

        let to = accounts
            .get_mut(&(ticker.to_string(), to.to_owned()))
            .unwrap();
        to.amount = to.amount + amount;
    }

    fn lock(&self, ticker: &str, addr: &AccountAddress, amount: u128) {
        let mut accounts = self.accounts.lock().unwrap();
        let acc = accounts
            .get_mut(&(ticker.to_string(), addr.to_owned()))
            .unwrap();
        if !acc.is_lockable {
            panic!("It is not lockable balance.");
        }

        acc.amount = acc.amount - amount;
    }

    fn unlock(&self, ticker: &str, addr: &AccountAddress, amount: u128) {
        let mut accounts = self.accounts.lock().unwrap();
        let acc = accounts
            .get_mut(&(ticker.to_string(), addr.to_owned()))
            .unwrap();
        if !acc.is_lockable {
            panic!("It is not lockable balance.");
        }

        acc.amount = acc.amount + amount;
    }
}

pub fn addr(addr: &str) -> AccountAddress {
    AccountAddress::from_hex_literal(addr).unwrap()
}

pub fn ident(ident: &str) -> Identifier {
    Identifier::new(ident).unwrap()
}