use std::{fs, path::Path};

use rusqlite::{params, Connection};

use crate::error::AppResult;

pub fn initialize_database(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let conn = open_connection(path)?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            extension TEXT NOT NULL,
            file_kind TEXT NOT NULL,
            workspace_kind TEXT NOT NULL,
            alias TEXT,
            title TEXT,
            content TEXT,
            target_url TEXT,
            size INTEGER NOT NULL DEFAULT 0,
            modified_at INTEGER NOT NULL DEFAULT 0,
            indexed_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_files_workspace_id ON files(workspace_id);
        CREATE INDEX IF NOT EXISTS idx_files_kind ON files(file_kind);
        CREATE INDEX IF NOT EXISTS idx_files_name ON files(file_name);
        "#,
    )?;

    migrate_alias_column(&conn)?;
    recreate_fts_table(&conn)?;
    conn.execute_batch("PRAGMA user_version = 2;")?;

    Ok(())
}

fn migrate_alias_column(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(files)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;

    let mut has_alias = false;
    for column in columns {
        if column? == "alias" {
            has_alias = true;
            break;
        }
    }

    if !has_alias {
        conn.execute("ALTER TABLE files ADD COLUMN alias TEXT", [])?;
    }

    Ok(())
}

fn recreate_fts_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS files_fts;

        CREATE VIRTUAL TABLE files_fts USING fts5(
            alias,
            file_name,
            title,
            content
        );
        "#,
    )?;

    let mut stmt = conn.prepare(
        r#"
        SELECT id, alias, file_name, title, content
        FROM files
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for row in rows {
        let (id, alias, file_name, title, content) = row?;
        conn.execute(
            r#"
            INSERT INTO files_fts(rowid, alias, file_name, title, content)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![id, alias, file_name, title, content],
        )?;
    }

    Ok(())
}

pub fn open_connection(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        "#,
    )?;
    Ok(conn)
}
