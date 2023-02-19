#![feature(iter_array_chunks)]
mod consts;
mod disk_entry;
mod fs_visitor;
mod fsevent;
mod models;
mod schema;
mod utils;

use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use consts::*;
use crossbeam_channel::bounded;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel_migrations::MigrationHarness;
use fsevent::EventId;
use models::DbMeta;
use models::DiskEntryRaw;
use std::time::Instant;
use tracing::info;

const DATABASE_URL: &str = std::env!("DATABASE_URL");

fn scan_fs(conn: &mut CardinalDbConnection) -> Result<()> {
    let (raw_entry_sender, raw_entry_receiver) = bounded(MAX_RAW_ENTRY_COUNT);

    std::thread::spawn(move || {
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
        let mut visitor_builder = fs_visitor::VisitorBuilder { raw_entry_sender };
        walkdir.visit(&mut visitor_builder);
    });

    let mut last_time = Instant::now();
    let mut insert_num = 0;
    let mut printed = 0;
    for entries in raw_entry_receiver.iter() {
        if insert_num - printed >= 100000 {
            info!(
                "insert: {}, speed: {}i/s, remaining: {}",
                insert_num,
                (insert_num - printed) as f32 / last_time.elapsed().as_secs_f32(),
                raw_entry_receiver.len(),
            );
            last_time = Instant::now();
            printed = insert_num;
        }
        insert_num += entries.len();
        conn.save_entries(&entries)
            .context("Save entries failed.")?;
    }

    Ok(())
}

struct CardinalDbConnection {
    conn: SqliteConnection,
}

impl CardinalDbConnection {
    fn connect() -> Result<Self> {
        let mut conn = SqliteConnection::establish(DATABASE_URL).with_context(|| {
            anyhow!(
                "Establish sqlite connection with url: `{}` failed.",
                DATABASE_URL
            )
        })?;
        conn.batch_execute(CONNECTION_PRAGMAS)
            .context("Run connection pragmas failed.")?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| anyhow!(e))
            .context("Run connection migrations failed.")?;
        Ok(Self { conn })
    }
    fn get_event_id(&mut self) -> Result<EventId> {
        use schema::db_meta::dsl::*;
        let event_id = db_meta
            .select(the_value)
            .filter(the_key.eq(b"event_id".to_vec()))
            .first::<Vec<u8>>(&mut self.conn)
            .context("Get event_id failed.")?;
        bincode::decode_from_slice(&event_id, CONFIG)
            .map(|(x, _)| x)
            .context("Decode event id failed.")
    }

    fn save_event_id(&mut self, event_id: &EventId) -> Result<()> {
        use schema::db_meta::dsl::*;
        let event_id =
            bincode::encode_to_vec(event_id, CONFIG).context("Encode event id failed.")?;
        let new_meta = DbMeta {
            the_key: b"event_id".to_vec(),
            the_value: event_id,
        };
        diesel::insert_into(db_meta)
            .values(&new_meta)
            .on_conflict(the_key)
            .do_update()
            .set(the_value.eq(&new_meta.the_value))
            .execute(&mut self.conn)
            .context("Upsert event id to db failed.")?;
        Ok(())
    }

    fn save_entries(&mut self, entries: &[DiskEntryRaw]) -> Result<()> {
        self.conn
            .transaction(|conn| {
                use schema::dir_entrys::dsl::*;
                for entry in entries.iter() {
                    let _num_insert = diesel::insert_into(dir_entrys)
                        .values(entry)
                        .on_conflict(the_path)
                        .do_update()
                        .set(the_meta.eq(&entry.the_meta))
                        .execute(conn)?;
                }
                Ok::<(), diesel::result::Error>(())
            })
            .context("Batch save entries failed.")
    }
}

/// The Database contains the file system snapshot and the time starting
/// to take the snapshot.
///
/// To make it really useful, merge the filesystem change(from start time to
/// current time) into the file system.
pub struct Database {
    /// The time starting to scan this file system tree.
    event_id: EventId,
    conn: CardinalDbConnection,
}

impl Database {
    pub fn from_fs() -> Result<Self> {
        let mut conn = CardinalDbConnection::connect().context("Get db connection failed.")?;
        let event_id = match conn.get_event_id() {
            Ok(x) => x,
            Err(e) => {
                info!(?e, "Event id fetching failed:");
                // scan_fs needs a lot of time, so event id should be gotten before it.
                let new_event_id = EventId::now();
                scan_fs(&mut conn).context("Scan fs failed.")?;
                conn.save_event_id(&new_event_id)
                    .context("Save current event id failed")?;
                new_event_id
            }
        };

        info!(?event_id, "The start event id");
        Ok(Self { event_id, conn })
    }
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("debug").init();
    let _ = std::fs::remove_file(DATABASE_URL);
    let db = Database::from_fs().unwrap();
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
