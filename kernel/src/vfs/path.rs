use alloc::sync::Arc;

use crate::vfs::errno::Errno;
use crate::vfs::inode::InodeOps;

pub fn lookup_path(root: Arc<dyn InodeOps>, path: &str) -> Result<Arc<dyn InodeOps>, Errno> {
    if !path.starts_with('/') {
        return Err(Errno::Invalid);
    }

    let mut current = root;

    for part in path.split('/').filter(|part| !part.is_empty()) {
        current = current.lookup(part)?;
    }

    Ok(current)
}
