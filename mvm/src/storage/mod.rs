use crate::storage::bank::{Balances, Bank};
use crate::storage::chain::{ExecutionContext, TxInfo};
use crate::storage::event::{EventHandler, EventWriter};
use crate::storage::session::Session;
use crate::storage::store::{DataAccess, RawData};
use move_vm_runtime::loader::Loader;

pub mod bank;
pub mod chain;
pub mod event;
pub mod session;
pub mod store;

pub struct NodeApi<S, E, B>
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    data_access: DataAccess<S>,
    event_writer: EventWriter<E>,
    bank: Bank<B>,
}

impl<S, E, B> NodeApi<S, E, B>
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    pub fn new(raw_data: S, event_handler: E, balances: B) -> NodeApi<S, E, B> {
        NodeApi {
            data_access: DataAccess::new(raw_data),
            event_writer: EventWriter::new(event_handler),
            bank: Bank::new(balances),
        }
    }

    pub fn new_session<'a, 't>(
        &'a self,
        loader: &'t Loader,
        tx_info: Option<TxInfo>,
    ) -> Session<'a, 't, S, E, B> {
        Session::new(
            &self.data_access,
            self.bank.new_session(loader),
            &self.event_writer,
            ExecutionContext::new(tx_info),
        )
    }
}
