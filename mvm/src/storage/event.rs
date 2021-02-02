use move_core_types::language_storage::TypeTag;
use move_core_types::value::MoveTypeLayout;
use move_core_types::vm_status::StatusCode;
use move_vm_types::natives::function::PartialVMError;
use move_vm_types::values::Value;
use vm::errors::{Location, VMError};

/// Event handler.
pub trait EventHandler {
    fn on_event(&self, guid: Vec<u8>, seq_num: u64, ty_tag: TypeTag, message: Vec<u8>);
}

pub type Event = (Vec<u8>, u64, TypeTag, MoveTypeLayout, Value);

pub struct EventWriter<E> {
    handler: E,
}

impl<E> EventWriter<E>
where
    E: EventHandler,
{
    pub fn new(handler: E) -> EventWriter<E> {
        EventWriter { handler }
    }

    pub fn write_event(&self, event: Event) -> Result<(), VMError> {
        let msg = event.4.simple_serialize(&event.3).ok_or_else(|| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .finish(Location::Undefined)
        })?;
        self.handler.on_event(event.0, event.1, event.2, msg);
        Ok(())
    }
}
