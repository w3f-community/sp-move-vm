use crate::storage::bank::Balances;
use crate::storage::chain::TxInfo;
use crate::storage::event::EventHandler;
use crate::storage::session::{Events, Session};
use crate::storage::store::RawData;
use crate::storage::NodeApi;
use crate::types::{Gas, ModuleTx, ScriptTx, VmResult};
use crate::vm_config::loader::load_vm_config;
use crate::Vm;
use anyhow::Error;
use move_core_types::gas_schedule::CostTable;
use move_core_types::gas_schedule::{AbstractMemorySize, GasAlgebra, GasUnits};
use move_core_types::vm_status::StatusCode;
use move_vm_runtime::data_cache::TransactionEffects;
use move_vm_runtime::logging::NoContextLog;
use move_vm_runtime::move_vm::MoveVM;
use move_vm_types::gas_schedule::CostStrategy;
use vm::errors::{Location, PartialVMError, VMError};
use vm::CompiledModule;

/// MoveVM.
pub struct Mvm<S, E, B>
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    vm: MoveVM,
    cost_table: CostTable,
    node_api: NodeApi<S, E, B>,
}

impl<S, E, B> Mvm<S, E, B>
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    /// Creates a new move vm with given store and event handler.
    pub fn new(store: S, event_handler: E, bank: B) -> Result<Mvm<S, E, B>, Error> {
        let config = load_vm_config(&store)?;

        Ok(Mvm {
            vm: MoveVM::new(),
            cost_table: config.gas_schedule,
            node_api: NodeApi::new(store, event_handler, bank),
        })
    }

    pub fn session(&self, tx_info: Option<TxInfo>) -> Session<'_, '_, S, E, B> {
        self.node_api.new_session(self.vm.loader(), tx_info)
    }

    /// Stores write set into storage and handle events.
    fn handle_tx_effects(
        &self,
        session: &Session<'_, '_, S, E, B>,
        tx_effects: TransactionEffects,
    ) -> Result<(), VMError> {
        for (addr, vals) in tx_effects.resources {
            for (struct_tag, ty_layout, tp, val_opt) in vals {
                match val_opt {
                    None => {
                        session.delete_resource(addr, struct_tag, ty_layout, tp)?;
                    }
                    Some(val) => {
                        session.insert_resource(addr, struct_tag, ty_layout, tp, val)?;
                    }
                };
            }
        }

        for (module_id, blob) in tx_effects.modules {
            session.publish_module(module_id, blob);
        }

        for event in tx_effects.events {
            session.write_event(event)?;
        }

        Ok(())
    }

    /// Handle vm result and return transaction status code.
    fn handle_vm_result(
        &self,
        state_session: &Session<'_, '_, S, E, B>,
        cost_strategy: CostStrategy,
        gas_meta: Gas,
        result: Result<TransactionEffects, VMError>,
    ) -> VmResult {
        let gas_used = GasUnits::new(gas_meta.max_gas_amount)
            .sub(cost_strategy.remaining_gas())
            .get();

        match result.and_then(|e| self.handle_tx_effects(state_session, e)) {
            Ok(_) => VmResult::new(StatusCode::EXECUTED, gas_used),
            Err(err) => {
                //todo log error.
                VmResult::new(err.major_status(), gas_used)
            }
        }
    }
}

impl<S, E, B> Vm for Mvm<S, E, B>
where
    S: RawData,
    E: EventHandler,
    B: Balances,
{
    fn publish_module(&self, gas: Gas, module: ModuleTx) -> VmResult {
        let (module, sender) = module.into_inner();

        let mut cost_strategy =
            CostStrategy::transaction(&self.cost_table, GasUnits::new(gas.max_gas_amount()));

        let state_session = self.node_api.new_session(self.vm.loader(), None);

        let result = cost_strategy
            .charge_intrinsic_gas(AbstractMemorySize::new(module.len() as u64))
            .and_then(|_| {
                CompiledModule::deserialize(&module)
                    .map_err(|e| e.finish(Location::Undefined))
                    .and_then(|compiled_module| {
                        let module_id = compiled_module.self_id();
                        if sender != *module_id.address() {
                            return Err(PartialVMError::new(
                                StatusCode::MODULE_ADDRESS_DOES_NOT_MATCH_SENDER,
                            )
                            .finish(Location::Module(module_id)));
                        }

                        cost_strategy
                            .charge_intrinsic_gas(AbstractMemorySize::new(module.len() as u64))?;

                        let mut session = self.vm.new_session(&state_session);
                        session
                            .publish_module(
                                module.to_vec(),
                                sender,
                                &mut cost_strategy,
                                &NoContextLog::new(),
                            )
                            .and_then(|_| session.finish())
                    })
            });
        self.handle_vm_result(&state_session, cost_strategy, gas, result)
    }

    fn execute_script(&self, gas: Gas, tx: ScriptTx) -> VmResult {
        let (script, args, type_args, senders, tx_info) = tx.into_inner();

        let state_session = self.node_api.new_session(self.vm.loader(), Some(tx_info));

        let mut session = self.vm.new_session(&state_session);

        let mut cost_strategy =
            CostStrategy::transaction(&self.cost_table, GasUnits::new(gas.max_gas_amount()));

        let result = session
            .execute_script(
                script,
                type_args,
                args,
                senders,
                &mut cost_strategy,
                &NoContextLog::new(),
            )
            .and_then(|_| session.finish());

        self.handle_vm_result(&state_session, cost_strategy, gas, result)
    }

    fn clear(&self) {
        self.vm.clear();
    }
}
