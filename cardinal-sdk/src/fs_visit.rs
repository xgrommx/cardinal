use bincode::Encode;
use rayon::{iter::ParallelBridge, prelude::ParallelIterator};
use serde::Serialize;
use std::{
    fs::{self, Metadata},
    io::Error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::UNIX_EPOCH,
};

#[derive(Serialize, Encode, Debug)]
pub struct Node {
    // TODO(ldm0): is this arc still needed?
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Arc<Node>>,
    pub data: NodeData,
}

#[derive(Serialize, Encode, Debug)]
pub struct NodeData {
    pub name: String,
    pub ctime: Option<u64>,
    pub mtime: Option<u64>,
}

impl NodeData {
    pub fn new(name: String, metadata: &Option<Metadata>) -> Self {
        let (ctime, mtime) = match metadata {
            Some(metadata) => ctime_mtime_from_metadata(metadata),
            None => (None, None),
        };
        Self { name, ctime, mtime }
    }
}

#[derive(Default, Debug)]
pub struct WalkData {
    pub num_files: AtomicUsize,
    pub num_dirs: AtomicUsize,
}

pub fn walk_it(dir: PathBuf, walk_data: &WalkData) -> Option<Node> {
    walk(dir, walk_data, 0)
}

fn ctime_mtime_from_metadata(metadata: &fs::Metadata) -> (Option<u64>, Option<u64>) {
    // TODO(ldm0): is this fast enough?
    let ctime = metadata
        .created()
        .ok()
        .and_then(|x| x.duration_since(UNIX_EPOCH).ok())
        .map(|x| x.as_secs());
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|x| x.duration_since(UNIX_EPOCH).ok())
        .map(|x| x.as_secs());
    (ctime, mtime)
}

fn walk(dir: PathBuf, walk_data: &WalkData, depth: usize) -> Option<Node> {
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
                            if let Ok(data) = entry.file_type() {
                                if data.is_dir() {
                                    return walk(entry.path(), walk_data, depth + 1);
                                } else {
                                    walk_data.num_files.fetch_add(1, Ordering::Relaxed);
                                    let metadata = entry.metadata().ok();
                                    let name = entry
                                        .path()
                                        .file_name()
                                        .and_then(|x| x.to_str())
                                        .map(|x| x.to_string())
                                        .unwrap_or_default();
                                    return Some(Node {
                                        children: vec![],
                                        data: NodeData::new(name, &metadata),
                                    });
                                }
                            }
                        }
                        Err(failed) => {
                            if handle_error_and_retry(failed) {
                                return walk(dir.clone(), walk_data, depth);
                            }
                        }
                    }
                    None
                })
                .map(Arc::new)
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
    Some(Node {
        children,
        data: NodeData::new(name, metadata),
    })
}

fn handle_error_and_retry(failed: &Error) -> bool {
    failed.kind() == std::io::ErrorKind::Interrupted
}
