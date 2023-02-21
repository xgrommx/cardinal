use crate::consts::CHUNK_SIZE;
use crate::disk_entry::DiskEntry;
use crate::models::DiskEntryRaw;
use anyhow::Context;
use anyhow::Result;
use crossbeam_channel::Sender;
use ignore::ParallelVisitor;
use ignore::ParallelVisitorBuilder;
use ignore::WalkState;

pub struct VisitorBuilder {
    pub raw_entry_sender: Sender<Vec<DiskEntryRaw>>,
}

impl<'s> ParallelVisitorBuilder<'s> for VisitorBuilder {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
        Box::new(Visitor {
            buffer: Vec::with_capacity(CHUNK_SIZE),
            raw_entry_sender: self.raw_entry_sender.clone(),
        })
    }
}

struct Visitor {
    raw_entry_sender: Sender<Vec<DiskEntryRaw>>,
    buffer: Vec<DiskEntryRaw>,
}

impl ParallelVisitor for Visitor {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
        if let Ok(entry) = entry {
            if let Ok(entry) = entry_to_raw(entry) {
                self.buffer.push(entry);
                if self.buffer.len() >= CHUNK_SIZE {
                    self.raw_entry_sender
                        .send(std::mem::take(&mut self.buffer))
                        .unwrap();
                }
            }
        }
        WalkState::Continue
    }
}

fn entry_to_raw(entry: ignore::DirEntry) -> Result<DiskEntryRaw> {
    let metadata = entry.metadata().context("Fetch metadata failed.")?;
    let entry = DiskEntry {
        path: entry.path().to_path_buf(),
        meta: metadata.into(),
    };
    entry.to_raw().context("Encode entry failed.")
}
