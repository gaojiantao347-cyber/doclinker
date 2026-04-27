use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use walkdir::{DirEntry, WalkDir};

use crate::{
    error::AppResult,
    models::{FileKind, IndexedFile, WorkspaceConfig},
};

pub fn scan_workspace(
    workspace: &WorkspaceConfig,
    exclude_patterns: &[String],
) -> AppResult<Vec<IndexedFile>> {
    let root = PathBuf::from(&workspace.path);
    let mut files = Vec::new();

    let mut iter = WalkDir::new(&root).into_iter();
    while let Some(entry) = iter.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                log::warn!("扫描文件失败: {err}");
                continue;
            }
        };

        if should_skip_entry(&entry, &root, exclude_patterns) {
            if entry.file_type().is_dir() {
                iter.skip_current_dir();
            }
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        if let Some(indexed) = parse_supported_file(workspace, entry.path())? {
            files.push(indexed);
        }
    }

    Ok(files)
}

pub fn parse_supported_file(
    workspace: &WorkspaceConfig,
    path: &Path,
) -> AppResult<Option<IndexedFile>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let file_kind = match extension.as_str() {
        "md" => FileKind::Markdown,
        "exe" => FileKind::Executable,
        "url" => FileKind::Url,
        value if is_text_extension(value) => FileKind::Text,
        _ => return Ok(None),
    };

    let metadata = fs::metadata(path)?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default();

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    let mut indexed = IndexedFile {
        workspace_id: workspace.id.clone(),
        workspace_kind: workspace.kind.clone(),
        file_kind,
        path: path.to_string_lossy().to_string(),
        file_name: file_name.clone(),
        extension,
        alias: None,
        title: Some(file_name),
        content: None,
        target_url: None,
        size: metadata.len() as i64,
        modified_at,
    };

    match indexed.file_kind {
        FileKind::Markdown => {
            let content = fs::read_to_string(path)?;
            indexed.alias = extract_markdown_alias(&content);
            indexed.title = extract_markdown_title(&content).or_else(|| indexed.title.clone());
            indexed.content = Some(content);
        }
        FileKind::Text => {
            indexed.content = Some(fs::read_to_string(path)?);
        }
        FileKind::Executable => {}
        FileKind::Url => {
            indexed.target_url = extract_url_target(path)?;
        }
    }

    Ok(Some(indexed))
}

fn is_text_extension(extension: &str) -> bool {
    matches!(
        extension,
        "txt" | "html" | "htm" | "json" | "xml" | "yaml" | "yml" | "csv" | "log"
    )
}

fn should_skip_entry(entry: &DirEntry, root: &Path, exclude_patterns: &[String]) -> bool {
    if entry.depth() == 0 {
        return false;
    }

    let relative = match entry.path().strip_prefix(root) {
        Ok(value) => value,
        Err(_) => return false,
    };

    relative.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        exclude_patterns
            .iter()
            .any(|pattern| pattern.eq_ignore_ascii_case(&value))
    })
}

fn extract_markdown_alias(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut aliases = Vec::new();
    let mut collecting_aliases = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }

        if let Some(value) = trimmed.strip_prefix("alias:") {
            collecting_aliases = false;
            collect_alias_values(value, &mut aliases);
            continue;
        }

        if let Some(value) = trimmed.strip_prefix("aliases:") {
            collecting_aliases = true;
            collect_alias_values(value, &mut aliases);
            continue;
        }

        if collecting_aliases {
            if let Some(value) = trimmed.strip_prefix('-') {
                collect_alias_values(value, &mut aliases);
            } else if !trimmed.is_empty() {
                collecting_aliases = false;
            }
        }
    }

    if aliases.is_empty() {
        None
    } else {
        Some(aliases.join(" "))
    }
}

fn collect_alias_values(value: &str, aliases: &mut Vec<String>) {
    let normalized = value
        .trim()
        .trim_matches('[')
        .trim_matches(']')
        .trim_matches('"')
        .trim_matches('\'');

    for part in normalized.split(',') {
        let alias = part.trim().trim_matches('"').trim_matches('\'').trim();
        if !alias.is_empty() {
            aliases.push(alias.to_string());
        }
    }
}

fn extract_markdown_title(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let title = trimmed.trim_start_matches('#').trim();
            if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            }
        } else {
            None
        }
    })
}

fn extract_url_target(path: &Path) -> AppResult<Option<String>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if let Some(value) = line.strip_prefix("URL=") {
            return Ok(Some(value.trim().to_string()));
        }
    }

    Ok(None)
}
