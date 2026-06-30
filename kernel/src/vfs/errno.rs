#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Errno {
    NotFound,
    NotDir,
    IsDir,
    Invalid,
    PermissionDenied,
    Unsupported,
}
