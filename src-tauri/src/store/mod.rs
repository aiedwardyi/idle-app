pub const SCHEMA: &str = include_str!("schema.sql");

#[cfg(test)]
mod tests {
    #[test]
    fn schema_executes_on_fresh_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn schema_version_stays_single_row_on_reapply() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let id: i64 = conn
            .query_row("SELECT id FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, 1);
    }
}
