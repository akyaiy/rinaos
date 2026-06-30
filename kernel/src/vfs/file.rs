use alloc::sync::Arc;

use crate::vfs::errno::Errno;
use crate::vfs::inode::{InodeOps, Metadata};

pub struct File {
    inode: Arc<dyn InodeOps>,
    offset: usize,
    readable: bool,
    writable: bool,
}

impl File {
    pub fn new(inode: Arc<dyn InodeOps>, readable: bool, writable: bool) -> Self {
        Self {
            inode,
            offset: 0,
            readable,
            writable,
        }
    }

    pub fn metadata(&self) -> Metadata {
        self.inode.metadata()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Errno> {
        if !self.readable {
            return Err(Errno::PermissionDenied);
        }

        let read = self.inode.read_at(self.offset, buf)?;
        self.offset += read;
        Ok(read)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, Errno> {
        if !self.writable {
            return Err(Errno::PermissionDenied);
        }

        let written = self.inode.write_at(self.offset, buf)?;
        self.offset += written;
        Ok(written)
    }
}
