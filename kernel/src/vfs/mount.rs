use alloc::sync::Arc;

use crate::vfs::errno::Errno;
use crate::vfs::file::File;
use crate::vfs::inode::{FileType, InodeOps};
use crate::vfs::path::lookup_path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenFlags {
    readable: bool,
    writable: bool,
}

impl OpenFlags {
    pub const RDONLY: Self = Self {
        readable: true,
        writable: false,
    };

    pub const WRONLY: Self = Self {
        readable: false,
        writable: true,
    };

    pub const RDWR: Self = Self {
        readable: true,
        writable: true,
    };
}

pub struct Vfs {
    root: Arc<dyn InodeOps>,
}

impl Vfs {
    pub fn new(root: Arc<dyn InodeOps>) -> Self {
        Self { root }
    }

    pub fn root(&self) -> Arc<dyn InodeOps> {
        Arc::clone(&self.root)
    }

    pub fn open(&self, path: &str, flags: OpenFlags) -> Result<File, Errno> {
        let inode = lookup_path(self.root(), path)?;

        if inode.metadata().file_type == FileType::Directory {
            return Err(Errno::IsDir);
        }

        Ok(File::new(inode, flags.readable, flags.writable))
    }
}
