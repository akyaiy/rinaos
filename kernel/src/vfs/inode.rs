use alloc::sync::Arc;

use crate::vfs::errno::Errno;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    Regular,
    Directory,
    CharDevice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub file_type: FileType,
    pub size: usize,
}

pub trait InodeOps: Send + Sync {
    fn metadata(&self) -> Metadata;

    fn lookup(&self, _name: &str) -> Result<Arc<dyn InodeOps>, Errno> {
        Err(Errno::NotDir)
    }

    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize, Errno> {
        Err(Errno::Unsupported)
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize, Errno> {
        Err(Errno::Unsupported)
    }
}
