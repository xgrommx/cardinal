use crate::schema::rows;
use diesel::Insertable;

#[derive(Clone, Insertable)]
#[diesel(table_name = rows)]
pub struct DiskEntryRaw {
    pub the_path: Vec<u8>,
    pub the_meta: Vec<u8>,
}
