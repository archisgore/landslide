
use crate::error::LandslideError;
use super::block::BlockState;
use sled::{Db};

struct State {
	db: Db,
	block_state: BlockState,
}


impl State {


	pub fn block_state(&self) -> BlockState {
		self.block_state()
	}

    pub async fn flush(&self) -> Result<(), LandslideError> {
		self.db.flush_async().await;
		Ok(())
	}
}
