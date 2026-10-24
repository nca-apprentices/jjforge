//! jj-lib's `OpHeadsStore` for `jf`: a file per head, as jj keeps them. The
//! forge's `HEAD` is never one of them. `jf` merges the forge's operations
//! into its own, and vcs moves `HEAD` itself.

use std::path::Path;

use async_trait::async_trait;
use jj_lib::backend::BackendInitError;
use jj_lib::op_heads_store::OpHeadsStore;
use jj_lib::op_heads_store::OpHeadsStoreError;
use jj_lib::op_heads_store::OpHeadsStoreLock;
use jj_lib::op_store::OperationId;
use jj_lib::simple_op_heads_store::SimpleOpHeadsStore;

/// jj-lib's own heads under the store type `jjforge`.
#[derive(Debug)]
pub(crate) struct Store(SimpleOpHeadsStore);

impl Store {
    pub(crate) fn init(
        dir: &Path,
        root_operation_id: &OperationId,
    ) -> Result<Self, BackendInitError> {
        Ok(Self(SimpleOpHeadsStore::init(dir, root_operation_id)?))
    }

    pub(crate) fn load(dir: &Path) -> Self {
        Self(SimpleOpHeadsStore::load(dir))
    }
}

#[async_trait]
impl OpHeadsStore for Store {
    fn name(&self) -> &str {
        crate::NAME
    }

    async fn update_op_heads(
        &self,
        old_ids: &[OperationId],
        new_id: &OperationId,
    ) -> Result<(), OpHeadsStoreError> {
        self.0.update_op_heads(old_ids, new_id).await
    }

    async fn get_op_heads(&self) -> Result<Vec<OperationId>, OpHeadsStoreError> {
        self.0.get_op_heads().await
    }

    async fn lock(&self) -> Result<Box<dyn OpHeadsStoreLock + '_>, OpHeadsStoreError> {
        self.0.lock().await
    }
}
