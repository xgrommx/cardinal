#[cfg(test)]
mod tests;

use crate::fsevent::FsEvent;

use std::ffi::OsString;
use std::io::{self, BufWriter};
use std::iter::Peekable;
use std::{
    fs::{self, File},
    io::prelude::*,
    io::BufReader,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use bincode::{config::Configuration, Decode, Encode};
use pathbytes::{o2b, p2b};
use serde::{Deserialize, Serialize};
use walkdir::{IntoIter, WalkDir};

#[derive(Serialize, Deserialize, Decode, Encode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum FileType {
    Dir,
    File,
    Symlink,
    Unknown,
}

impl From<fs::FileType> for FileType {
    fn from(file_type: fs::FileType) -> Self {
        if file_type.is_dir() {
            FileType::Dir
        } else if file_type.is_file() {
            FileType::File
        } else if file_type.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Unknown
        }
    }
}

/// Most of the useful information for a disk node.
#[derive(Serialize, Deserialize, Decode, Encode, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Metadata {
    file_type: FileType,
    len: u64,
    created: SystemTime,
    modified: SystemTime,
    accessed: SystemTime,
    permissions_read_only: bool,
}

impl From<fs::Metadata> for Metadata {
    fn from(meta: fs::Metadata) -> Self {
        Self {
            file_type: meta.file_type().into(),
            len: meta.len(),
            created: meta.created().unwrap(),
            modified: meta.modified().unwrap(),
            accessed: meta.accessed().unwrap(),
            permissions_read_only: meta.permissions().readonly(),
        }
    }
}

#[derive(
    Serialize, Deserialize, Decode, Encode, Default, Clone, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct DiskEntry {
    /// WTF-8
    name: Vec<u8>,
    /// Is None when no permission.
    metadata: Option<Metadata>,
    /// Is set to Some when entry is a folder.
    entries: Vec<DiskEntry>,
}

impl DiskEntry {
    pub fn merge(&mut self, event: &FsEvent) {
        unimplemented!()
    }

    pub fn from_fs(path: &Path) -> DiskEntry {
        fn scan_folder(
            walker: &mut Peekable<DiskWalker>,
            parent_path: &Path,
            entry: &mut DiskEntry,
        ) {
            let DiskEntry { entries, .. } = entry;
            loop {
                // if a node under parent node.
                let under_parent = walker
                    .peek()
                    .map(|(path, _)| path.starts_with(parent_path))
                    .unwrap_or_default();
                if !under_parent {
                    break;
                }
                let (path, metadata) = match walker.next() {
                    Some(x) => x,
                    None => break,
                };
                // Should never panic since walkdir shouldn't emit same path twice.
                assert_ne!(path, parent_path);
                // Should never panic since root we are scanning after root.
                let mut entry = DiskEntry {
                    name: o2b(path.file_name().expect("a root path")).to_vec(),
                    metadata,
                    entries: Vec::new(),
                };
                scan_folder(walker, &path, &mut entry);
                entries.push(entry);
            }
        }

        let mut walker = DiskWalker::new(path).peekable();
        let (root_path, metadata) = walker.next().unwrap();
        assert_eq!(root_path, path);
        let mut entry = DiskEntry {
            name: p2b(path).to_vec(),
            metadata,
            entries: Vec::new(),
        };
        scan_folder(&mut walker, path, &mut entry);
        entry
    }
}

pub struct DiskWalker {
    walk_dir: Peekable<IntoIter>,
}

impl Iterator for DiskWalker {
    /// Metadata is none when permission denied.
    type Item = (PathBuf, Option<Metadata>);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.walk_dir.next()? {
                Ok(entry) => {
                    let meta = entry.metadata().ok().map(|x| x.into());
                    let path = entry.into_path();
                    break Some((path, meta));
                }
                Err(e) => {
                    if let Some(path) = e.path() {
                        // If we are trying to scan the inner elements in permission
                        // denied folder, walkdir will return a error with the path
                        // of the folder, here we filter out the dir path emitted in
                        // this situation.
                        if let Ok(x) = fs::symlink_metadata(path) {
                            if !x.is_dir() {
                                break Some((path.to_owned(), None));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DiskWalker {
    pub fn new(path: &Path) -> Self {
        Self {
            walk_dir: WalkDir::new(path).into_iter().peekable(),
        }
    }
}

/*
pub struct Volumes {
    pub entries: Vec<Entry>,
}

impl Volumes {
    pub fn from_fs() -> Result<Volumes> {
        let entries = if let Ok(file) = File::open("target/hierarchy.db") {
            let mut reader = BufReader::new(file);
            //let mut reader = flate2::read::ZlibDecoder::new(reader);
            bincode::decode_from_std_read(&mut reader, Configuration::standard())
                .context("decode from db failed.")?
        } else {
            scan_hierarchy()
        };
        Ok(Volumes { entries })
    }

    pub fn to_fs(&self) -> Result<()> {
        let file = File::create("target/hierarchy.db").context("create db failed")?;
        let mut file = BufWriter::new(file);
        // let mut file = flate2::write::ZlibEncoder::new(file, flate2::Compression::fast());
        bincode::encode_into_std_write(&self.entries, &mut file, Configuration::standard())
            .context("Write to db failed.")?;
        Ok(())
    }
}
*/
