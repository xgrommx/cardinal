use std::ffi::OsStr;
use std::path::Path;

#[cfg(target_family = "windows")]
pub fn p2b(path: &Path) -> &[u8] {
    // Path->OsStr->Slice->Wtf8->[u8]
    unsafe { std::mem::transmute(path) }
}
#[cfg(target_family = "unix")]
pub fn p2b(path: &Path) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes()
}
#[cfg(target_family = "wasm")]
pub fn p2b(path: &Path) -> &[u8] {
    use std::os::wasm::ffi::OsStrExt;
    path.as_os_str().as_bytes()
}

#[cfg(target_family = "windows")]
pub fn o2b(s: &OsStr) -> &[u8] {
    // OsStr->Slice->Wtf8->[u8]
    unsafe { std::mem::transmute(s) }
}
#[cfg(target_family = "unix")]
pub fn o2b(s: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes()
}
#[cfg(target_family = "wasm")]
pub fn o2b(s: &OsStr) -> &[u8] {
    use std::os::wasm::ffi::OsStrExt;
    s.as_bytes()
}

#[cfg(target_family = "windows")]
pub fn b2p(bytes: &[u8]) -> &Path {
    // Path->OsStr->Slice->Wtf8->[u8]
    unsafe { std::mem::transmute(bytes) }
}
#[cfg(target_family = "unix")]
pub fn b2p(bytes: &[u8]) -> &Path {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    Path::new(OsStr::from_bytes(bytes))
}
#[cfg(target_family = "wasm")]
pub fn b2p(bytes: &[u8]) -> &Path {
    use std::ffi::OsStr;
    use std::os::wasm::ffi::OsStrExt;
    Path::new(OsStr::from_bytes(bytes))
}
