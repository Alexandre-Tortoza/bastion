use refinery::embed_migrations;

embed_migrations!("src/migrations");

pub fn run(conn: &mut rusqlite::Connection) -> Result<(), crate::error::StoreError> {
    migrations::runner()
        .run(conn)
        .map_err(|e| crate::error::StoreError::Migration(e.to_string()))?;
    Ok(())
}
