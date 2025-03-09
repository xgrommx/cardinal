use std::ffi::OsStr;
use std::path::Path;

pub fn p2b(path: &Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}

pub fn o2b(s: &OsStr) -> &[u8] {
    s.as_encoded_bytes()
}

pub unsafe fn b2p(bytes: &[u8]) -> &Path {
    Path::new(OsStr::from_encoded_bytes_unchecked(bytes))
}
