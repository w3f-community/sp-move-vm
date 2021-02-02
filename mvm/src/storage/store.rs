use crate::access_path::AccessPath;
use move_core_types::account_address::AccountAddress;
use move_core_types::language_storage::{ModuleId, ResourceKey, StructTag};
use move_vm_runtime::data_cache::RemoteCache;
use vm::errors::{PartialVMResult, VMResult};

/// Storage access trait.
pub trait RawData {
    /// Returns the data for `key` in the storage or `None` if the key can not be found.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    /// Set `key` to `value` in the storage.
    fn insert(&self, key: &[u8], value: &[u8]);
    /// Clear the storage of the given `key` and its value.
    fn remove(&self, key: &[u8]);
}

pub struct DataAccess<D: RawData> {
    data_access: D,
}

impl<D> DataAccess<D>
where
    D: RawData,
{
    pub fn new(raw_data: D) -> DataAccess<D> {
        DataAccess {
            data_access: raw_data,
        }
    }

    pub fn get_by_path(&self, path: AccessPath) -> Option<Vec<u8>> {
        let mut key = Vec::with_capacity(AccountAddress::LENGTH + path.path.len());
        key.extend_from_slice(&path.address.to_u8());
        key.extend_from_slice(&path.path);
        self.data_access.get(&key)
    }
}

impl<S> RemoteCache for DataAccess<S>
where
    S: RawData,
{
    fn get_module(&self, module_id: &ModuleId) -> VMResult<Option<Vec<u8>>> {
        let path = AccessPath::from(module_id);
        Ok(self.get_by_path(path))
    }

    fn get_resource(
        &self,
        address: &AccountAddress,
        tag: &StructTag,
    ) -> PartialVMResult<Option<Vec<u8>>> {
        let path = AccessPath::resource_access_path(&ResourceKey::new(*address, tag.to_owned()));
        Ok(self.get_by_path(path))
    }
}

/// Trait provides storage modification functions.
pub trait DataMutator {
    /// Delete data blob by associate with given path.
    fn delete(&self, path: AccessPath);
    /// Insert data blob with associate path.
    fn insert(&self, path: AccessPath, blob: Vec<u8>);
}

impl<D> DataMutator for DataAccess<D>
where
    D: RawData,
{
    fn delete(&self, path: AccessPath) {
        let mut key = Vec::with_capacity(AccountAddress::LENGTH + path.path.len());
        key.extend_from_slice(&path.address.to_u8());
        key.extend_from_slice(&path.path);
        self.data_access.remove(&key);
    }

    fn insert(&self, path: AccessPath, blob: Vec<u8>) {
        let mut key = Vec::with_capacity(AccountAddress::LENGTH + path.path.len());
        key.extend_from_slice(&path.address.to_u8());
        key.extend_from_slice(&path.path);
        self.data_access.insert(&key, &blob);
    }
}
