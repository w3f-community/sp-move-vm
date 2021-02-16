use crate::storage::session::{Resolve, ResolverResult};
use core::cell::RefCell;
use core::ops::Deref;
use diem_crypto::Lazy;
use hashbrown::HashMap;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::TypeTag::Struct;
use move_core_types::language_storage::{StructTag, TypeTag, CORE_CODE_ADDRESS};
use move_core_types::value::MoveTypeLayout;
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::loader::Loader;
use move_vm_types::loaded_data::runtime_types::{StructType, Type};
use move_vm_types::values::{Container, Value, ValueImpl};
use std::rc::Rc;
use vm::errors::{Location, PartialVMError, VMError};

pub static ACCOUNT_MODULE: Lazy<Identifier> = Lazy::new(|| Identifier::new("Account").unwrap());
pub static BALANCE_STRUCT: Lazy<Identifier> = Lazy::new(|| Identifier::new("Balance").unwrap());

pub static DFI_MODULE: Lazy<Identifier> = Lazy::new(|| Identifier::new("Dfinance").unwrap());
pub static T_STRUCT: Lazy<Identifier> = Lazy::new(|| Identifier::new("T").unwrap());

#[derive(Debug, Clone)]
pub struct Account {
    /// Balance amount.
    pub amount: u128,
    /// Shows whether it is possible to block the amount on the account.
    pub is_lockable: bool,
}

/// Balances access trait.
pub trait Balances {
    /// Returns balance by given key and account address.
    fn get_balance(&self, ticker: &str, addr: &AccountAddress) -> Option<Account>;
    /// Transfers `amount` coins with `ticker` from address `from` to address `to`.
    fn transfer(&self, ticker: &str, from: &AccountAddress, to: &AccountAddress, amount: u128);
    /// Locks `amount` coins with `ticker` on address `addr`.
    fn lock(&self, ticker: &str, addr: &AccountAddress, amount: u128);
    /// Unlocks `amount` coins with `ticker` on address `addr`.
    fn unlock(&self, ticker: &str, addr: &AccountAddress, amount: u128);
}

pub struct Bank<B>
where
    B: Balances,
{
    cache: BalanceHandlerCache,
    bank: B,
}

impl<B> Bank<B>
where
    B: Balances,
{
    pub fn new(bank: B) -> Bank<B> {
        Bank {
            cache: Default::default(),
            bank,
        }
    }

    pub fn new_session<'t>(&self, loader: &'t Loader) -> BankSession<'_, 't, B> {
        BankSession::new(&self, loader)
    }
}

pub struct BankSession<'a, 't, B>
where
    B: Balances,
{
    bank: &'a Bank<B>,
    balances: RefCell<HashMap<AccountAddress, HashMap<String, Account>>>,
    type_viewer: TypeWalker<'t>,
}

impl<B> BankSession<'_, '_, B>
where
    B: Balances,
{
    fn new<'a, 't>(bank: &'a Bank<B>, loader: &'t Loader) -> BankSession<'a, 't, B> {
        BankSession {
            bank,
            balances: RefCell::new(Default::default()),
            type_viewer: TypeWalker::new(loader),
        }
    }

    fn make_handlers(
        &self,
        tag: &StructTag,
        tp: &Type,
    ) -> Result<Option<Rc<BalanceHandler>>, VMError> {
        Ok(
            if let Some(handler) = &self.bank.cache.get_balance_handler(tag) {
                handler.clone()
            } else {
                self.bank
                    .cache
                    .store_balance_handler(tag, self.type_viewer.find_balance(tag, tp)?)
            },
        )
    }

    pub fn handle_delete_balance(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
        ty: &MoveTypeLayout,
        tp: Type,
    ) -> Result<bool, VMError> {
        if let Some(handler) = self.make_handlers(tag, &tp)? {
            Ok(handler.is_unlocked())
        } else {
            Ok(false)
        }
    }

    pub fn handle_insert_balance(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
        ty: &MoveTypeLayout,
        tp: Type,
        value: &Value,
    ) -> Result<bool, VMError> {
        if let Some(handler) = self.make_handlers(tag, &tp)? {
            dbg!(handler.resolve_balance(&value.0)?);
            Ok(handler.is_unlocked())
        } else {
            Ok(false)
        }
    }
}

fn is_balance(tag: &StructTag) -> bool {
    if tag.address == CORE_CODE_ADDRESS
        && &tag.module == ACCOUNT_MODULE.deref()
        && &tag.name == BALANCE_STRUCT.deref()
        && tag.type_params.len() == 1
    {
        match &tag.type_params[0] {
            TypeTag::Bool
            | TypeTag::U8
            | TypeTag::U64
            | TypeTag::U128
            | TypeTag::Address
            | TypeTag::Signer
            | TypeTag::Vector(_) => false,
            Struct(tag) => tag.address == CORE_CODE_ADDRESS && &tag.module == DFI_MODULE.deref(),
        }
    } else {
        false
    }
}

fn is_coin(tp: &StructType) -> bool {
    tp.module.address() == &CORE_CODE_ADDRESS
        && tp.module.name() == DFI_MODULE.as_ref()
        && &tp.name == T_STRUCT.deref()
}

impl<B> Resolve for BankSession<'_, '_, B>
where
    B: Balances,
{
    fn resolve(&self, address: &AccountAddress, tag: &StructTag) -> ResolverResult {
        if is_balance(tag) {
            let ticker = match tag.type_params.get(0) {
                Some(Struct(tag)) => tag.name.as_str(),
                _ => {
                    return ResolverResult::Resolved(Err(PartialVMError::new(
                        StatusCode::INTERNAL_TYPE_ERROR,
                    )));
                }
            };

            {
                let balances = self.balances.borrow();
                let balance = balances
                    .get(address)
                    .and_then(|map| map.get(ticker))
                    .map(|acc| acc.amount.to_le_bytes().to_vec());
                if let Some(balance) = balance {
                    return ResolverResult::Resolved(Ok(Some(balance)));
                }
            }

            let balance = match self.bank.bank.get_balance(ticker, address) {
                Some(balance) => {
                    let encoded = balance.amount.to_le_bytes().to_vec();
                    let mut balances = self.balances.borrow_mut();
                    let entry = balances.entry(*address);
                    let acc = entry.or_default();
                    acc.insert(ticker.to_owned(), balance);
                    Some(encoded)
                }
                None => None,
            };

            ResolverResult::Resolved(Ok(balance))
        } else {
            ResolverResult::Unresolved
        }
    }
}

pub struct TypeWalker<'a> {
    loader: &'a Loader,
}

impl<'a> TypeWalker<'a> {
    pub fn new(loader: &'a Loader) -> TypeWalker<'a> {
        TypeWalker { loader }
    }

    pub fn find_balance(
        &self,
        tag: &StructTag,
        tp: &Type,
    ) -> Result<Option<BalanceHandler>, VMError> {
        Ok(if is_balance(tag) {
            let ticker = match tag.type_params.get(0) {
                Some(Struct(tag)) => tag.name.as_str(),
                _ => {
                    return Err(PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR)
                        .finish(Location::Undefined));
                }
            };
            Some(BalanceHandler::Unlocked(Rc::new(ticker.to_owned())))
        } else {
            let balances = self.find_in_type(tp, &tag.type_params);
            if balances.is_empty() {
                None
            } else {
                Some(BalanceHandler::Locked(balances))
            }
        })
    }

    fn find_in_type(&self, tp: &Type, tp_tags: &[TypeTag]) -> Vec<(Rc<String>, Vec<usize>)> {
        match tp {
            Type::Vector(tp) => return self.find_in_type(tp, tp_tags),
            Type::Struct(index) => {
                let struct_tp = self.loader.struct_at(*index);
                let mut res = vec![];
                for (index, field) in struct_tp.fields.iter().enumerate() {
                    for (ticker, mut path) in self.find_in_type(field, tp_tags) {
                        path.insert(0, index);
                        res.push((ticker, path));
                    }
                }
                return res;
            }
            Type::StructInstantiation(index, tp_params) => {
                let inner_tp_tags = self
                    .loader
                    .struct_gidx_to_type_tag(*index, tp_params)
                    .ok()
                    .map(|st| st.type_params);

                let struct_tp = self.loader.struct_at(*index);
                return if is_coin(&struct_tp) && tp_params.len() == 1 {
                    let ticker = match &tp_params[0] {
                        Type::Struct(index) => {
                            let struct_tp = self.loader.struct_at(*index);
                            struct_tp.name.as_str().to_owned()
                        }
                        Type::TyParam(index) => match &tp_tags[*index] {
                            TypeTag::Struct(st) => st.name.as_str().to_owned(),
                            _ => "_".to_owned(),
                        },
                        Type::Bool => "bool".to_owned(),
                        Type::U8 => "u8".to_owned(),
                        Type::U64 => "u64".to_owned(),
                        Type::U128 => "u128".to_owned(),
                        Type::Address => "address".to_owned(),
                        Type::Signer => "signer".to_owned(),
                        Type::Vector(_) => "vector".to_owned(),
                        Type::StructInstantiation(index, _) => {
                            let struct_tp = self.loader.struct_at(*index);
                            struct_tp.name.as_str().to_owned()
                        }
                        Type::Reference(_) => "reference".to_owned(),
                        Type::MutableReference(_) => "reference".to_owned(),
                    };
                    vec![(Rc::new(ticker), vec![0])]
                } else {
                    let mut res = vec![];
                    for (index, field) in struct_tp.fields.iter().enumerate() {
                        let field = match inner_tp_tags.as_ref() {
                            Some(tp_tags) => self.find_in_type(field, tp_tags),
                            None => self.find_in_type(field, tp_tags),
                        };

                        for (ticker, mut path) in field {
                            path.insert(0, index);
                            res.push((ticker, path));
                        }
                    }
                    res
                };
            }
            Type::TyParam(index) => match &tp_tags[*index] {
                TypeTag::Struct(st) => {
                    if let Some(gidx) = self.loader.struct_tag_to_struct_gidx(st).ok() {
                        return self.find_in_type(&Type::Struct(gidx), st.type_params.as_ref());
                    }
                }
                _ => {}
            },
            _ => {}
        }
        vec![]
    }
}

#[derive(Debug)]
pub enum BalanceHandler {
    Locked(Vec<(Rc<String>, Vec<usize>)>),
    Unlocked(Rc<String>),
}

impl BalanceHandler {
    pub fn is_unlocked(&self) -> bool {
        match self {
            BalanceHandler::Locked(_) => false,
            BalanceHandler::Unlocked(_) => true,
        }
    }

    pub fn resolve_balance(&self, val: &ValueImpl) -> Result<Vec<Balance>, VMError> {
        match self {
            BalanceHandler::Locked(tickers) => {
                let mut balances = vec![];

                for (ticker, path) in tickers {
                    balances.extend(
                        Self::load_value(path, val)?
                            .into_iter()
                            .map(|balance| Balance {
                                ticker: ticker.clone(),
                                balance,
                                locked: true,
                            })
                            .collect::<Vec<_>>(),
                    );
                }

                Ok(balances)
            }
            BalanceHandler::Unlocked(ticker) => Ok(Self::load_value(&[0, 0], val)?
                .into_iter()
                .map(|balance| Balance {
                    ticker: ticker.clone(),
                    balance,
                    locked: false,
                })
                .collect::<Vec<_>>()),
        }
    }

    fn container(path: &[usize], cr: &Container) -> Result<Vec<u128>, VMError> {
        match cr {
            Container::Locals(vals) => {
                let val = &vals.borrow()[path[0]];
                Self::load_value(&path[1..], val)
            }
            Container::VecR(vals) => {
                let mut res = vec![];
                for val in vals.borrow().iter() {
                    res.extend(Self::load_value(&path, val)?);
                }
                Ok(res)
            }
            Container::VecC(vals) => {
                let mut res = vec![];
                for val in vals.borrow().iter() {
                    res.extend(Self::load_value(&path, val)?);
                }

                Ok(res)
            }
            Container::StructR(vals) => {
                let val = &vals.borrow()[path[0]];
                Self::load_value(&path[1..], val)
            }
            Container::StructC(vals) => {
                let val = &vals.borrow()[path[0]];
                Self::load_value(&path[1..], val)
            }
            Container::VecU8(_)
            | Container::VecU64(_)
            | Container::VecU128(_)
            | Container::VecBool(_)
            | Container::VecAddress(_) => Err(type_err()),
        }
    }

    fn load_value(path: &[usize], val: &ValueImpl) -> Result<Vec<u128>, VMError> {
        match &val {
            ValueImpl::U128(val) => {
                if !path.is_empty() {
                    return Err(type_err());
                } else {
                    Ok(vec![*val])
                }
            }
            ValueImpl::Container(cr) => Self::container(path, cr),
            ValueImpl::ContainerRef(rf) => Self::container(path, rf.container()),
            ValueImpl::IndexedRef(rf) => Self::container(path, rf.container_ref.container()),
            ValueImpl::Address(_)
            | ValueImpl::Bool(_)
            | ValueImpl::U8(_)
            | ValueImpl::U64(_)
            | ValueImpl::Invalid => Err(type_err()),
        }
    }
}

fn type_err() -> VMError {
    PartialVMError::new(StatusCode::INTERNAL_TYPE_ERROR).finish(Location::Undefined)
}

#[derive(Debug)]
pub struct Balance {
    ticker: Rc<String>,
    balance: u128,
    locked: bool,
}

pub struct BalanceHandlerCache {
    cache: RefCell<HashMap<StructTag, Option<Rc<BalanceHandler>>>>,
}

impl BalanceHandlerCache {
    pub fn new() -> BalanceHandlerCache {
        BalanceHandlerCache {
            cache: RefCell::new(Default::default()),
        }
    }

    pub fn get_balance_handler(&self, tag: &StructTag) -> Option<Option<Rc<BalanceHandler>>> {
        self.cache.borrow().get(tag).map(|rf| rf.clone())
    }

    pub fn store_balance_handler(
        &self,
        tag: &StructTag,
        handlers: Option<BalanceHandler>,
    ) -> Option<Rc<BalanceHandler>> {
        match handlers {
            None => {
                self.cache.borrow_mut().insert(tag.clone(), None);
                None
            }
            Some(handlers) => {
                let handlers = Rc::new(handlers);
                self.cache
                    .borrow_mut()
                    .insert(tag.clone(), Some(handlers.clone()));
                Some(handlers)
            }
        }
    }

    pub fn clear(&self) {
        self.cache.borrow_mut().clear();
    }
}

impl Default for BalanceHandlerCache {
    fn default() -> Self {
        Self::new()
    }
}
