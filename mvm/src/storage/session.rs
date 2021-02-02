use crate::access_path::AccessPath;
use crate::storage::bank::{Balances, BankSession};
use crate::storage::chain::ExecutionContext;
use crate::storage::event::{EventHandler, EventWriter, Event};
use crate::storage::store::{DataAccess, DataMutator, RawData};
use move_core_types::account_address::AccountAddress;
use move_core_types::language_storage::{ModuleId, StructTag};
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::data_cache::RemoteCache;
use move_vm_types::natives::function::PartialVMError;
use vm::errors::{PartialVMResult, VMResult, VMError, Location};
use move_core_types::value::MoveTypeLayout;
use move_vm_types::values::Value;
use move_vm_types::loaded_data::runtime_types::Type;

pub struct Session<'a, 't, S, E, B>
    where
        S: RawData,
        E: EventHandler,
        B: Balances,
{
    data_access: &'a DataAccess<S>,
    event_writer: &'a EventWriter<E>,
    bank: BankSession<'a, 't, B>,
    context: ExecutionContext,
}

impl<S, E, B> Session<'_, '_, S, E, B>
    where
        S: RawData,
        E: EventHandler,
        B: Balances,
{
    pub fn new<'a, 't>(
        data_access: &'a DataAccess<S>,
        bank: BankSession<'a, 't, B>,
        event_writer: &'a EventWriter<E>,
        context: ExecutionContext,
    ) -> Session<'a, 't, S, E, B> {
        Session {
            data_access,
            event_writer,
            bank,
            context,
        }
    }

    pub fn delete_resource(&self, address: AccountAddress, tag: StructTag, ty: MoveTypeLayout, tp: Type)  -> Result<(), VMError> {
        if !self.bank.handle_delete_balance(&address, &tag, &ty, tp)? {
            self.data_access.delete(AccessPath::new(address, tag.access_vector()));
        }
        Ok(())
    }

    pub fn insert_resource(&self, address: AccountAddress, tag: StructTag, ty: MoveTypeLayout, tp: Type, value: Value) -> Result<(), VMError> {
        if !self.bank.handle_insert_balance(&address, &tag, &ty, tp, &value)? {
            let ap = AccessPath::new(address, tag.access_vector());
            let blob = value.simple_serialize(&ty).ok_or_else(|| {
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .finish(Location::Undefined)
            })?;
            self.data_access.insert(ap, blob);
        }
        Ok(())
    }

    pub fn publish_module(&self, id: ModuleId, blob: Vec<u8>) {
        self.data_access.insert(
            AccessPath::new(*id.address(), id.access_vector()),
            blob,
        );
    }
}


pub trait Events<E> where E: EventHandler {
    fn write_event(&self, event: Event) -> Result<(), VMError>;
}

impl<S, E, B> Events<E> for Session<'_, '_, S, E, B>
    where
        S: RawData,
        E: EventHandler,
        B: Balances,
{
    fn write_event(&self, event: Event) -> Result<(), VMError> {
        self.event_writer.write_event(event)
    }
}

pub enum ResolverResult {
    Resolved(PartialVMResult<Option<Vec<u8>>>),
    Unresolved,
}

pub trait Resolve {
    fn resolve(&self, address: &AccountAddress, tag: &StructTag) -> ResolverResult;
}

impl<S, E, B> RemoteCache for Session<'_, '_, S, E, B>
    where
        S: RawData,
        E: EventHandler,
        B: Balances,
{
    fn get_module(&self, module_id: &ModuleId) -> VMResult<Option<Vec<u8>>> {
        self.data_access.get_module(module_id)
    }

    fn get_resource(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
    ) -> PartialVMResult<Option<Vec<u8>>> {
        if let ResolverResult::Resolved(result) = self.context.resolve(address, tag) {
            return result;
        }

        if let ResolverResult::Resolved(result) = self.bank.resolve(address, tag) {
            return result;
        }

        self.data_access.get_resource(address, tag)
    }
}

