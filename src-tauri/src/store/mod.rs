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
}
