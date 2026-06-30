use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::drivers::console;
use crate::vfs::errno::Errno;
use crate::vfs::fs::FileSystem;
use crate::vfs::inode::{FileType, InodeOps, Metadata};

pub trait CharDevice: Send + Sync {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, Errno> {
        Err(Errno::Unsupported)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, Errno> {
        Err(Errno::Unsupported)
    }
}

pub struct DevFs {
    root: Arc<DevDir>,
}

struct DevDir {
    entries: Vec<DevEntry>,
}

struct DevEntry {
    name: &'static str,
    inode: Arc<dyn InodeOps>,
}

struct DevNode {
    device: Arc<dyn CharDevice>,
}

struct NullDevice;
struct ConsoleDevice;

impl DevFs {
    pub fn new() -> Self {
        let mut root = DevDir::new();

        root.add("null", Arc::new(DevNode::new(Arc::new(NullDevice))));
        root.add("console", Arc::new(DevNode::new(Arc::new(ConsoleDevice))));

        Self {
            root: Arc::new(root),
        }
    }
}

impl FileSystem for DevFs {
    fn root_inode(&self) -> Arc<dyn InodeOps> {
        self.root.clone()
    }
}

impl DevDir {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, name: &'static str, inode: Arc<dyn InodeOps>) {
        self.entries.push(DevEntry { name, inode });
    }
}

impl InodeOps for DevDir {
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

impl DevNode {
    fn new(device: Arc<dyn CharDevice>) -> Self {
        Self { device }
    }
}

impl InodeOps for DevNode {
    fn metadata(&self) -> Metadata {
        Metadata {
            file_type: FileType::CharDevice,
            size: 0,
        }
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, Errno> {
        self.device.read(buf)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, Errno> {
        self.device.write(buf)
    }
}

impl CharDevice for NullDevice {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, Errno> {
        Ok(0)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        Ok(buf.len())
    }
}

impl CharDevice for ConsoleDevice {
    fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        for byte in buf {
            console::put_byte(*byte);
        }

        Ok(buf.len())
    }
}
