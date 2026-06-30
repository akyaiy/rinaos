extern crate alloc;

pub mod errno;
pub mod file;
pub mod fs;
pub mod inode;
pub mod mount;
pub mod path;

use crate::fs::rootfs::RootFs;
use crate::sync::spinlock::IrqSpinLock;
use crate::vfs::errno::Errno;
use crate::vfs::file::File;
use crate::vfs::fs::FileSystem;
use crate::vfs::mount::Vfs;

pub use mount::OpenFlags;

static VFS: IrqSpinLock<Option<Vfs>> = IrqSpinLock::new(None);

pub fn init() {
    let rootfs = RootFs::new();
    *VFS.lock() = Some(Vfs::new(rootfs.root_inode()));
}

pub fn open(path: &str, flags: OpenFlags) -> Result<File, Errno> {
    let guard = VFS.lock();
    let vfs = guard.as_ref().ok_or(Errno::Invalid)?;

    vfs.open(path, flags)
}
