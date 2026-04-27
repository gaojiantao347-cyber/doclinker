use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::{AppError, AppResult},
    models::{FilePreviewDto, IndexedFile, SearchResultDto, WorkspaceConfig},
    scanner,
};

pub fn rebuild_workspace(
    conn: &mut Connection,
    workspace: &WorkspaceConfig,
    exclude_patterns: &[String],
) -> AppResult<()> {
    let files = scanner::scan_workspace(workspace, exclude_patterns)?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM files_fts WHERE rowid IN (SELECT id FROM files WHERE workspace_id = ?1)",
        params![workspace.id],
    )?;
    tx.execute(
        "DELETE FROM files WHERE workspace_id = ?1",
        params![workspace.id],
    )?;

    for file in files {
        upsert_file_tx(&tx, &file)?;
    }

    tx.commit()?;
    Ok(())
}

pub fn index_file(conn: &mut Connection, file: &IndexedFile) -> AppResult<()> {
    let tx = conn.transaction()?;
    upsert_file_tx(&tx, file)?;
    tx.commit()?;
    Ok(())
}

pub fn index_path(
    conn: &mut Connection,
    workspace: &WorkspaceConfig,
    path: &Path,
) -> AppResult<()> {
    if let Some(file) = scanner::parse_supported_file(workspace, path)? {
        index_file(conn, &file)?;
    }
    Ok(())
}

pub fn remove_path(conn: &mut Connection, path: &Path) -> AppResult<()> {
    let tx = conn.transaction()?;
    if let Some(id) = tx
        .query_row(
            "SELECT id FROM files WHERE path = ?1",
            params![path.to_string_lossy().to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        tx.execute("DELETE FROM files_fts WHERE rowid = ?1", params![id])?;
        tx.execute("DELETE FROM files WHERE id = ?1", params![id])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn rename_path(
    conn: &mut Connection,
    workspace: &WorkspaceConfig,
    old_path: &Path,
    new_path: &Path,
) -> AppResult<()> {
    remove_path(conn, old_path)?;
    if new_path.exists() {
        index_path(conn, workspace, new_path)?;
    }
    Ok(())
}

pub fn search(conn: &Connection, keyword: &str, limit: usize) -> AppResult<Vec<SearchResultDto>> {
    let query = build_fts_query(keyword);
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        r#"
        SELECT
            files.path,
            files.file_name,
            files.alias,
            files.title,
            files.file_kind,
            files.workspace_kind,
            files.target_url,
            bm25(files_fts, 10.0, 7.0, 4.0, 1.0) AS score
        FROM files_fts
        JOIN files ON files.id = files_fts.rowid
        WHERE files_fts MATCH ?1
        ORDER BY score, files.modified_at DESC
        LIMIT ?2
        "#,
    )?;

    let rows = stmt.query_map(params![query, limit as i64], |row| {
        Ok(SearchResultDto {
            path: row.get(0)?,
            file_name: row.get(1)?,
            alias: row.get(2)?,
            title: row.get(3)?,
            file_kind: row.get(4)?,
            workspace_kind: row.get(5)?,
            target_url: row.get(6)?,
            score: row.get(7)?,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

pub fn read_preview(conn: &Connection, path: &str) -> AppResult<FilePreviewDto> {
    conn.query_row(
        r#"
        SELECT
            path,
            file_name,
            alias,
            title,
            file_kind,
            workspace_kind,
            target_url,
            content
        FROM files
        WHERE path = ?1 AND file_kind IN ('markdown', 'text')
        "#,
        params![path],
        |row| {
            Ok(FilePreviewDto {
                path: row.get(0)?,
                file_name: row.get(1)?,
                alias: row.get(2)?,
                title: row.get(3)?,
                file_kind: row.get(4)?,
                workspace_kind: row.get(5)?,
                target_url: row.get(6)?,
                content: row.get(7)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::message("未找到可预览的文件"))
}

fn upsert_file_tx(tx: &rusqlite::Transaction<'_>, file: &IndexedFile) -> AppResult<()> {
    let now = current_unix_ts();
    tx.execute(
        r#"
        INSERT INTO files (
            workspace_id,
            path,
            file_name,
            extension,
            file_kind,
            workspace_kind,
            alias,
            title,
            content,
            target_url,
            size,
            modified_at,
            indexed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(path) DO UPDATE SET
            workspace_id = excluded.workspace_id,
            file_name = excluded.file_name,
            extension = excluded.extension,
            file_kind = excluded.file_kind,
            workspace_kind = excluded.workspace_kind,
            alias = excluded.alias,
            title = excluded.title,
            content = excluded.content,
            target_url = excluded.target_url,
            size = excluded.size,
            modified_at = excluded.modified_at,
            indexed_at = excluded.indexed_at
        "#,
        params![
            file.workspace_id,
            file.path,
            file.file_name,
            file.extension,
            file.file_kind.as_str(),
            file.workspace_kind.as_str(),
            file.alias,
            file.title,
            file.content,
            file.target_url,
            file.size,
            file.modified_at,
            now,
        ],
    )?;

    let row_id = tx.query_row(
        "SELECT id FROM files WHERE path = ?1",
        params![file.path],
        |row| row.get::<_, i64>(0),
    )?;

    tx.execute("DELETE FROM files_fts WHERE rowid = ?1", params![row_id])?;
    tx.execute(
        r#"
        INSERT INTO files_fts(rowid, alias, file_name, title, content)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![row_id, file.alias, file.file_name, file.title, file.content],
    )?;

    Ok(())
}

fn build_fts_query(keyword: &str) -> String {
    keyword
        .split_whitespace()
        .map(|part| format!("\"{}\"*", part.replace('"', " ")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn current_unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
