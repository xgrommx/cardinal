use bincode::Encode;
use rayon::{iter::ParallelBridge, prelude::ParallelIterator};
use serde::Serialize;
use std::{
    fs,
    io::Error,
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
    walk(dir, walk_data, 0)
}

fn walk(dir: &Path, walk_data: &WalkData, depth: usize) -> Option<Node> {
    if walk_data.ignore_directory.as_deref() == Some(dir) {
        return None;
    }
    let metadata = &dir.metadata().ok();
    let children = if metadata.as_ref().map(|x| x.is_dir()).unwrap_or_default() {
        walk_data.num_dirs.fetch_add(1, Ordering::Relaxed);
        let read_dir = fs::read_dir(&dir);
        match read_dir {
            Ok(entries) => entries
                .into_iter()
                .par_bridge()
                .filter_map(|entry| {
                    match &entry {
                        Ok(entry) => {
                            if walk_data.ignore_directory.as_deref() == Some(dir) {
                                return None;
                            }
                            if let Ok(data) = entry.file_type() {
                                if data.is_dir() {
                                    return walk(&entry.path(), walk_data, depth + 1);
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
                                return walk(dir, walk_data, depth);
                            }
                        }
                    }
                    None
                })
                .collect(),
            Err(failed) => {
                if handle_error_and_retry(&failed) {
                    return walk(dir, walk_data, depth);
                } else {
                    vec![]
                }
            }
        }
    } else {
        walk_data.num_files.fetch_add(1, Ordering::Relaxed);
        vec![]
    };
    let name = dir
        .file_name()
        .and_then(|x| x.to_str())
        .map(|x| x.to_string())
        .unwrap_or_default();
    Some(Node { children, name })
}

fn handle_error_and_retry(failed: &Error) -> bool {
    failed.kind() == std::io::ErrorKind::Interrupted
}
