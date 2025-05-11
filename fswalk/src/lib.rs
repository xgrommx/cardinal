use bincode::Encode;
use rayon::{iter::ParallelBridge, prelude::ParallelIterator};
use serde::Serialize;
use std::{
    fs,
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Serialize, Encode, Debug)]
pub struct Node {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    pub name: String,
}

#[derive(Default, Debug)]
pub struct WalkData {
    pub num_files: AtomicUsize,
    pub num_dirs: AtomicUsize,
    ignore_directory: Option<PathBuf>,
}

impl WalkData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ignore_directory(path: PathBuf) -> Self {
        Self {
            ignore_directory: Some(path),
            ..Default::default()
        }
    }
}

pub fn walk_it(dir: &Path, walk_data: &WalkData) -> Option<Node> {
    walk(dir, walk_data)
}

fn walk(path: &Path, walk_data: &WalkData) -> Option<Node> {
    if walk_data.ignore_directory.as_deref() == Some(path) {
        return None;
    }
    let metadata = match path.metadata() {
        Ok(metadata) => Some(metadata),
        // If it's not found, we definitely don't want it.
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        // If it's permission denied or something, we still want to insert it into the tree.
        Err(e) => {
            if handle_error_and_retry(&e) {
                path.metadata().ok()
            } else {
                None
            }
        }
    };
    let children = if metadata.as_ref().map(|x| x.is_dir()).unwrap_or_default() {
        walk_data.num_dirs.fetch_add(1, Ordering::Relaxed);
        let read_dir = fs::read_dir(&path);
        match read_dir {
            Ok(entries) => entries
                .into_iter()
                .par_bridge()
                .filter_map(|entry| {
                    match &entry {
                        Ok(entry) => {
                            if walk_data.ignore_directory.as_deref() == Some(path) {
                                return None;
                            }
                            if let Ok(data) = entry.file_type() {
                                if data.is_dir() {
                                    return walk(&entry.path(), walk_data);
                                } else {
                                    walk_data.num_files.fetch_add(1, Ordering::Relaxed);
                                    let name = entry
                                        .path()
                                        .file_name()
                                        .and_then(|x| x.to_str())
                                        .map(|x| x.to_string())
                                        .unwrap_or_default();
                                    return Some(Node {
                                        children: vec![],
                                        name,
                                    });
                                }
                            }
                        }
                        Err(failed) => {
                            if handle_error_and_retry(failed) {
                                return walk(path, walk_data);
                            }
                        }
                    }
                    None
                })
                .collect(),
            Err(failed) => {
                if handle_error_and_retry(&failed) {
                    return walk(path, walk_data);
                } else {
                    vec![]
                }
            }
        }
    } else {
        walk_data.num_files.fetch_add(1, Ordering::Relaxed);
        vec![]
    };
    let name = path
        .file_name()
        .and_then(|x| x.to_str())
        .map(|x| x.to_string())
        .unwrap_or_default();
    Some(Node { children, name })
}

fn handle_error_and_retry(failed: &Error) -> bool {
    failed.kind() == std::io::ErrorKind::Interrupted
}
