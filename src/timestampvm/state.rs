
use crate::error::LandslideError;
use crate::id::Id;
use super::block::{StorageBlock, Block};
use sled::{Db, Tree};

trait SingletonState {
	fn is_initialized() -> Result<bool, LandslideError>;
	fn set_initialized() -> Result<(), LandslideError>;
}



trait State: SingletonState {
    fn commit() -> Result<(), LandslideError>;
	fn close() -> Result<(), LandslideError>;
}

struct BlockState {
	// block database
	blockDB: Tree,
	lastAccepted: Id,
}

impl BlockState {
	fn get_block(&self, block_id: Id) -> Result<Option<StorageBlock>, LandslideError> {
		let maybe_storage_block_bytes = self.blockDB.get(block_id)?;

		let storage_block_bytes = match maybe_storage_block_bytes {
			Some(s) => s,
			None => return Ok(None),
		};

		let sb: StorageBlock = serde_json::from_slice(&storage_block_bytes)?;

		Ok(Some(sb))
	}

	fn put_block(&self, block: StorageBlock) -> Result<(), LandslideError> {
		let block_bytes = serde_json::to_vec(&block)?;
		self.blockDB.insert(block.id()?, block_bytes)?;
		Ok(())
	}

	fn delete_block(&self, block_id: Id) -> Result<(), LandslideError> {
		self.blockDB.remove(block_id)?;
		Ok(())
	}

	fn get_last_accepted(&self) -> Result<ID, LandslideError> {
		todo!();

	}
	fn set_last_accepted(&self, id: ID) -> Result<(), LandslideError> {
		todo!();

	}
}

struct StateImpl {
	db: Db,
}