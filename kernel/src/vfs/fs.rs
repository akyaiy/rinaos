use alloc::sync::Arc;

use crate::vfs::inode::InodeOps;

pub trait FileSystem {
    fn root_inode(&self) -> Arc<dyn InodeOps>;
}
