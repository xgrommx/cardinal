#![feature(iter_array_chunks)]
mod models;
mod schema;

use anyhow::Context;
use anyhow::Result;
use bincode::{Decode, Encode};
use crossbeam_channel::bounded;
use crossbeam_channel::Sender;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use models::DiskEntryRaw;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::time::SystemTime;
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../migrations");

fn main() -> Result<()> {
    let _ = std::fs::remove_file("target/rows.db");
    let mut conn =
        SqliteConnection::establish("target/rows.db").context("Get sqlite connection failed.")?;
    conn.batch_execute(
        "PRAGMA synchronous = OFF; PRAGMA journal_mode = WAL; PRAGMA temp_store = MEMORY;",
    )
    .unwrap();
    conn.run_pending_migrations(MIGRATIONS).unwrap();

    const CHUNK_SIZE: usize = 1000;

    const MAX_RAW_ENTRY_SIZE: usize = 5 * 1024 * 1024;
    const MAX_RAW_ENTRY_COUNT: usize =
        MAX_RAW_ENTRY_SIZE / std::mem::size_of::<DiskEntryRaw>() / CHUNK_SIZE;
    let (raw_entry_sender, raw_entry_receiver) = bounded(MAX_RAW_ENTRY_COUNT);

    fn entry_to_raw(entry: ignore::DirEntry) -> Result<DiskEntryRaw> {
        let metadata = entry.metadata().context("Fetch metadata failed.")?;
        let entry = DiskEntry {
            path: entry.path().to_path_buf(),
            meta: metadata.into(),
        };
        entry.try_into().context("Encode entry failed.")
    }

    std::thread::spawn(move || {
        use ignore::ParallelVisitor;
        use ignore::ParallelVisitorBuilder;
        use ignore::WalkState;
        struct VisitorBuilder {
            raw_entry_sender: Sender<Vec<DiskEntryRaw>>,
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

        let threads = num_cpus::get_physical();
        dbg!(threads);
        let walkdir = ignore::WalkBuilder::new("/")
            .follow_links(false)
            .git_exclude(false)
            .git_global(false)
            .git_ignore(false)
            .hidden(false)
            .ignore(false)
            .ignore_case_insensitive(false)
            .max_depth(None)
            .max_filesize(None)
            .parents(false)
            .require_git(false)
            .same_file_system(true)
            .skip_stdout(false)
            .standard_filters(false)
            .threads(threads)
            .build_parallel();
        let mut visitor_builder = VisitorBuilder { raw_entry_sender };
        walkdir.visit(&mut visitor_builder);
    });

    let mut last_time = Instant::now();
    for (i, entrys) in raw_entry_receiver.iter().enumerate() {
        let n = 100;
        if i % n == 0 && i != 0 {
            println!(
                "insert: {}, speed: {}i/s, remaining: {}",
                i * CHUNK_SIZE,
                (n * CHUNK_SIZE) as f32 / last_time.elapsed().as_secs_f32(),
                raw_entry_receiver.len(),
            );
            last_time = Instant::now();
        }
        conn.transaction(|conn| {
            use schema::rows::dsl::*;
            for entry in entrys {
                let _num_insert = diesel::insert_into(rows)
                    .values(&entry)
                    .on_conflict(the_path)
                    .do_update()
                    .set(the_meta.eq(&entry.the_meta))
                    .execute(conn)?;
            }
            Ok::<(), diesel::result::Error>(())
        })?;
    }

    Ok(())
}

#[derive(Encode, Decode, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
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
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Metadata {
    pub file_type: FileType,
    pub len: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub accessed: SystemTime,
    pub permissions_read_only: bool,
}

impl From<fs::Metadata> for Metadata {
    fn from(meta: fs::Metadata) -> Self {
        // unwrap is legal here since these things are always available on PC platforms.
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

struct DiskEntry {
    path: PathBuf,
    meta: Metadata,
}

const CONFIG: bincode::config::Configuration = bincode::config::standard();

impl TryFrom<DiskEntryRaw> for DiskEntry {
    type Error = bincode::error::DecodeError;
    fn try_from(entry: DiskEntryRaw) -> Result<Self, Self::Error> {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let (meta, _) = bincode::decode_from_slice(&entry.the_meta, CONFIG)?;
        Ok(Self {
            path: OsString::from_vec(entry.the_path).into(),
            meta,
        })
    }
}

impl TryFrom<DiskEntry> for DiskEntryRaw {
    type Error = bincode::error::EncodeError;
    fn try_from(entry: DiskEntry) -> Result<Self, Self::Error> {
        use std::os::unix::ffi::OsStringExt;
        let the_meta = bincode::encode_to_vec(&entry.meta, CONFIG)?;
        Ok(Self {
            the_path: entry.path.into_os_string().into_vec(),
            the_meta,
        })
    }
}

/*
async fn add_row(
    conn: &mut SqliteConnection,
    DiskEntryRaw { the_path, the_meta }: DiskEntryRaw,
) -> Result<()> {
    sqlx::query!(
        r#"
INSERT INTO rows (the_path, the_meta)
VALUES (?,?)
ON CONFLICT(the_path) DO UPDATE SET the_meta = excluded.the_meta
        "#,
        the_path,
        the_meta
    )
    .execute(conn)
    .await
    .context("Upsert disk entry failed.")?;
    Ok(())
}

async fn get_row(pool: &SqlitePool, path: &[u8]) -> Result<DiskEntryRaw> {
    let mut conn = pool.acquire().await?;
    let row = sqlx::query_as!(
        DiskEntryRaw,
        r#"
SELECT the_path, the_meta
FROM rows
WHERE the_path = ?
        "#,
        path
    )
    .fetch_one(&mut conn)
    .await
    .context("Fetch from db failed.")?;
    Ok(row)
}

 */
