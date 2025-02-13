use crate::database::{
    storage::{
        ContractsStateMerkleData,
        ContractsStateMerkleMetadata,
        SparseMerkleMetadata,
    },
    Column,
    Database,
};
use fuel_core_storage::{
    tables::ContractsState,
    Error as StorageError,
    Mappable,
    MerkleRoot,
    MerkleRootStorage,
    StorageAsMut,
    StorageAsRef,
    StorageInspect,
    StorageMutate,
};
use fuel_core_types::{
    fuel_merkle::sparse::{
        in_memory,
        MerkleTree,
    },
    fuel_types::ContractId,
};
use std::{
    borrow::{
        BorrowMut,
        Cow,
    },
    ops::Deref,
};

impl StorageInspect<ContractsState> for Database {
    type Error = StorageError;

    fn get(
        &self,
        key: &<ContractsState as Mappable>::Key,
    ) -> Result<Option<Cow<<ContractsState as Mappable>::OwnedValue>>, Self::Error> {
        self.get(key.as_ref(), Column::ContractsState)
            .map_err(Into::into)
    }

    fn contains_key(
        &self,
        key: &<ContractsState as Mappable>::Key,
    ) -> Result<bool, Self::Error> {
        self.contains_key(key.as_ref(), Column::ContractsState)
            .map_err(Into::into)
    }
}

impl StorageMutate<ContractsState> for Database {
    fn insert(
        &mut self,
        key: &<ContractsState as Mappable>::Key,
        value: &<ContractsState as Mappable>::Value,
    ) -> Result<Option<<ContractsState as Mappable>::OwnedValue>, Self::Error> {
        let prev = Database::insert(self, key.as_ref(), Column::ContractsState, value)
            .map_err(Into::into);

        // Get latest metadata entry for this contract id
        let prev_metadata = self
            .storage::<ContractsStateMerkleMetadata>()
            .get(key.contract_id())?
            .unwrap_or_default();

        let root = prev_metadata.root;
        let storage = self.borrow_mut();
        let mut tree: MerkleTree<ContractsStateMerkleData, _> = {
            if root == [0; 32] {
                // The tree is empty
                MerkleTree::new(storage)
            } else {
                // Load the tree saved in metadata
                MerkleTree::load(storage, &root)
                    .map_err(|err| StorageError::Other(err.into()))?
            }
        };

        // Update the contract's key-value dataset. The key is the state key and
        // the value is the 32 bytes
        tree.update(key.state_key().deref(), value.as_slice())
            .map_err(|err| StorageError::Other(err.into()))?;

        // Generate new metadata for the updated tree
        let root = tree.root();
        let metadata = SparseMerkleMetadata { root };
        self.storage::<ContractsStateMerkleMetadata>()
            .insert(key.contract_id(), &metadata)?;

        prev
    }

    fn remove(
        &mut self,
        key: &<ContractsState as Mappable>::Key,
    ) -> Result<Option<<ContractsState as Mappable>::OwnedValue>, Self::Error> {
        let prev = Database::remove(self, key.as_ref(), Column::ContractsState)
            .map_err(Into::into);

        // Get latest metadata entry for this contract id
        let prev_metadata = self
            .storage::<ContractsStateMerkleMetadata>()
            .get(key.contract_id())?;

        if let Some(prev_metadata) = prev_metadata {
            let root = prev_metadata.root;

            // Load the tree saved in metadata
            let storage = self.borrow_mut();
            let mut tree: MerkleTree<ContractsStateMerkleData, _> =
                MerkleTree::load(storage, &root)
                    .map_err(|err| StorageError::Other(err.into()))?;

            // Update the contract's key-value dataset. The key is the state key and
            // the value is the 32 bytes
            tree.delete(key.state_key().deref())
                .map_err(|err| StorageError::Other(err.into()))?;

            let root = tree.root();
            if root == in_memory::MerkleTree::new().root() {
                // The tree is now empty; remove the metadata
                self.storage::<ContractsStateMerkleMetadata>()
                    .remove(key.contract_id())?;
            } else {
                // Generate new metadata for the updated tree
                let metadata = SparseMerkleMetadata { root };
                self.storage::<ContractsStateMerkleMetadata>()
                    .insert(key.contract_id(), &metadata)?;
            }
        }

        prev
    }
}

impl MerkleRootStorage<ContractId, ContractsState> for Database {
    fn root(&self, parent: &ContractId) -> Result<MerkleRoot, Self::Error> {
        let metadata = self.storage::<ContractsStateMerkleMetadata>().get(parent)?;
        let root = metadata
            .map(|metadata| metadata.root)
            .unwrap_or_else(|| in_memory::MerkleTree::new().root());
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuel_core_storage::{
        StorageAsMut,
        StorageAsRef,
    };
    use fuel_core_types::fuel_types::Bytes32;

    #[test]
    fn get() {
        let key = (&ContractId::from([1u8; 32]), &Bytes32::from([1u8; 32])).into();
        let stored_value: Bytes32 = Bytes32::from([2u8; 32]);

        let database = &mut Database::default();
        database
            .storage::<ContractsState>()
            .insert(&key, &stored_value)
            .unwrap();

        assert_eq!(
            *database
                .storage::<ContractsState>()
                .get(&key)
                .unwrap()
                .unwrap(),
            stored_value
        );
    }

    #[test]
    fn put() {
        let key = (&ContractId::from([1u8; 32]), &Bytes32::from([1u8; 32])).into();
        let stored_value: Bytes32 = Bytes32::from([2u8; 32]);

        let database = &mut Database::default();
        database
            .storage::<ContractsState>()
            .insert(&key, &stored_value)
            .unwrap();

        let returned: Bytes32 = *database
            .storage::<ContractsState>()
            .get(&key)
            .unwrap()
            .unwrap();
        assert_eq!(returned, stored_value);
    }

    #[test]
    fn remove() {
        let key = (&ContractId::from([1u8; 32]), &Bytes32::from([1u8; 32])).into();
        let stored_value: Bytes32 = Bytes32::from([2u8; 32]);

        let database = &mut Database::default();
        database
            .storage::<ContractsState>()
            .insert(&key, &stored_value)
            .unwrap();

        database.storage::<ContractsState>().remove(&key).unwrap();

        assert!(!database
            .storage::<ContractsState>()
            .contains_key(&key)
            .unwrap());
    }

    #[test]
    fn exists() {
        let key = (&ContractId::from([1u8; 32]), &Bytes32::from([1u8; 32])).into();
        let stored_value: Bytes32 = Bytes32::from([2u8; 32]);

        let database = &mut Database::default();
        database
            .storage::<ContractsState>()
            .insert(&key, &stored_value)
            .unwrap();

        assert!(database
            .storage::<ContractsState>()
            .contains_key(&key)
            .unwrap());
    }

    #[test]
    fn root() {
        let key = (&ContractId::from([1u8; 32]), &Bytes32::from([1u8; 32])).into();
        let stored_value: Bytes32 = Bytes32::from([2u8; 32]);

        let mut database = Database::default();

        StorageMutate::<ContractsState>::insert(&mut database, &key, &stored_value)
            .unwrap();

        let root = database.storage::<ContractsState>().root(key.contract_id());
        assert!(root.is_ok())
    }

    #[test]
    fn root_returns_empty_root_for_invalid_contract() {
        let invalid_contract_id = ContractId::from([1u8; 32]);
        let database = Database::default();
        let empty_root = in_memory::MerkleTree::new().root();
        let root = database
            .storage::<ContractsState>()
            .root(&invalid_contract_id)
            .unwrap();
        assert_eq!(root, empty_root)
    }

    #[test]
    fn put_updates_the_state_merkle_root_for_the_given_contract() {
        let contract_id = ContractId::from([1u8; 32]);
        let database = &mut Database::default();

        // Write the first contract state
        let state_key = Bytes32::from([1u8; 32]);
        let state: Bytes32 = Bytes32::from([0xff; 32]);
        let key = (&contract_id, &state_key).into();
        database
            .storage::<ContractsState>()
            .insert(&key, &state)
            .unwrap();

        // Read the first Merkle root
        let root_1 = database
            .storage::<ContractsState>()
            .root(&contract_id)
            .unwrap();

        // Write the second contract state
        let state_key = Bytes32::from([2u8; 32]);
        let state: Bytes32 = Bytes32::from([0xff; 32]);
        let key = (&contract_id, &state_key).into();
        database
            .storage::<ContractsState>()
            .insert(&key, &state)
            .unwrap();

        // Read the second Merkle root
        let root_2 = database
            .storage::<ContractsState>()
            .root(&contract_id)
            .unwrap();

        assert_ne!(root_1, root_2);
    }

    #[test]
    fn remove_updates_the_state_merkle_root_for_the_given_contract() {
        let contract_id = ContractId::from([1u8; 32]);
        let database = &mut Database::default();

        // Write the first contract state
        let state_key = Bytes32::new([1u8; 32]);
        let state: Bytes32 = Bytes32::from([0xff; 32]);
        let key = (&contract_id, &state_key).into();
        database
            .storage::<ContractsState>()
            .insert(&key, &state)
            .unwrap();
        let root_0 = database
            .storage::<ContractsState>()
            .root(&contract_id)
            .unwrap();

        // Write the second contract state
        let state_key = Bytes32::new([2u8; 32]);
        let state: Bytes32 = Bytes32::from([0xff; 32]);
        let key = (&contract_id, &state_key).into();
        database
            .storage::<ContractsState>()
            .insert(&key, &state)
            .unwrap();

        // Read the first Merkle root
        let root_1 = database
            .storage::<ContractsState>()
            .root(&contract_id)
            .unwrap();

        // Remove the first contract state
        let state_key = Bytes32::new([2u8; 32]);
        let key = (&contract_id, &state_key).into();
        database.storage::<ContractsState>().remove(&key).unwrap();

        // Read the second Merkle root
        let root_2 = database
            .storage::<ContractsState>()
            .root(&contract_id)
            .unwrap();

        assert_ne!(root_1, root_2);
        assert_eq!(root_0, root_2);
    }
}
