use crate::database::{
    storage::DatabaseColumn,
    Column,
    Database,
};
use fuel_core_storage::{
    not_found,
    tables::{
        FuelBlocks,
        SealedBlockConsensus,
    },
    Result as StorageResult,
    StorageAsRef,
};
use fuel_core_types::{
    blockchain::{
        consensus::{
            Consensus,
            Genesis,
            Sealed,
        },
        primitives::{
            BlockHeight,
            BlockId,
        },
        SealedBlock,
        SealedBlockHeader,
    },
    fuel_tx::Transaction,
};

impl DatabaseColumn for SealedBlockConsensus {
    fn column() -> Column {
        Column::FuelBlockConsensus
    }
}

impl Database {
    pub fn get_sealed_block_by_id(
        &self,
        block_id: &BlockId,
    ) -> StorageResult<Option<SealedBlock>> {
        // combine the block and consensus metadata into a sealed fuel block type

        let block = self.get_full_block(block_id)?;
        let consensus = self.storage::<SealedBlockConsensus>().get(block_id)?;

        if let (Some(block), Some(consensus)) = (block, consensus) {
            let sealed_block = SealedBlock {
                entity: block,
                consensus: consensus.into_owned(),
            };

            Ok(Some(sealed_block))
        } else {
            Ok(None)
        }
    }

    /// Returns `SealedBlock` by `height`.
    /// Reusable across different trait implementations
    pub fn get_sealed_block_by_height(
        &self,
        height: &BlockHeight,
    ) -> StorageResult<Option<SealedBlock>> {
        let block_id = match self.get_block_id(height)? {
            Some(i) => i,
            None => return Ok(None),
        };
        self.get_sealed_block_by_id(&block_id)
    }

    pub fn get_genesis(&self) -> StorageResult<Genesis> {
        let (_, genesis_block_id) = self.ids_of_genesis_block()?;
        let consensus = self
            .storage::<SealedBlockConsensus>()
            .get(&genesis_block_id)?
            .map(|c| c.into_owned());

        if let Some(Consensus::Genesis(genesis)) = consensus {
            Ok(genesis)
        } else {
            Err(not_found!(SealedBlockConsensus))
        }
    }

    pub fn get_sealed_block_header_by_height(
        &self,
        height: &BlockHeight,
    ) -> StorageResult<Option<SealedBlockHeader>> {
        let block_id = match self.get_block_id(height)? {
            Some(i) => i,
            None => return Ok(None),
        };
        self.get_sealed_block_header(&block_id)
    }

    pub fn get_sealed_block_header(
        &self,
        block_id: &BlockId,
    ) -> StorageResult<Option<SealedBlockHeader>> {
        let header = self.storage::<FuelBlocks>().get(block_id)?;
        let consensus = self.storage::<SealedBlockConsensus>().get(block_id)?;

        if let (Some(header), Some(consensus)) = (header, consensus) {
            let sealed_block = SealedBlockHeader {
                entity: header.into_owned().header().clone(),
                consensus: consensus.into_owned(),
            };

            Ok(Some(sealed_block))
        } else {
            Ok(None)
        }
    }

    pub fn get_transactions_on_block(
        &self,
        block_id: &BlockId,
    ) -> StorageResult<Option<Vec<Transaction>>> {
        Ok(self
            .get_sealed_block_by_id(block_id)?
            .map(|Sealed { entity: block, .. }| block.into_inner().1))
    }
}
