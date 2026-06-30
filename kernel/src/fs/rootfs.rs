use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::fs::devfs::DevFs;
use crate::vfs::errno::Errno;
use crate::vfs::fs::FileSystem;
use crate::vfs::inode::{FileType, InodeOps, Metadata};

pub struct RootFs {
    root: Arc<StaticDir>,
}

struct StaticDir {
    entries: Vec<DirEntry>,
}

struct DirEntry {
    name: &'static str,
    inode: Arc<dyn InodeOps>,
}

impl RootFs {
    pub fn new() -> Self {
        let mut root = StaticDir::new();
        let devfs = DevFs::new();

        root.add("dev", devfs.root_inode());

        Self {
            root: Arc::new(root),
        }
    }
}

impl FileSystem for RootFs {
    fn root_inode(&self) -> Arc<dyn InodeOps> {
        self.root.clone()
    }
}

impl StaticDir {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, name: &'static str, inode: Arc<dyn InodeOps>) {
        self.entries.push(DirEntry { name, inode });
    }
}

impl InodeOps for StaticDir {
    fn metadata(&self) -> Metadata {
        Metadata {
            file_type: FileType::Directory,
            size: self.entries.len(),
        }
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn InodeOps>, Errno> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| Arc::clone(&entry.inode))
            .ok_or(Errno::NotFound)
    }
}
