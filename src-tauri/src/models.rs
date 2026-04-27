use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceKind {
    Project,
    Doc,
    Scripts,
}

impl WorkspaceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Doc => "doc",
            Self::Scripts => "scripts",
        }
    }
}

impl std::fmt::Display for WorkspaceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: WorkspaceKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub workspaces: Vec<WorkspaceConfig>,
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub launch_on_startup: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            workspaces: Vec::new(),
            launch_on_startup: false,
            exclude_patterns: vec![
                "node_modules".into(),
                ".git".into(),
                "dist".into(),
                "target".into(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Markdown,
    Text,
    Executable,
    Url,
}

impl FileKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Text => "text",
            Self::Executable => "executable",
            Self::Url => "url",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub workspace_id: String,
    pub workspace_kind: WorkspaceKind,
    pub file_kind: FileKind,
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub alias: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub target_url: Option<String>,
    pub size: i64,
    pub modified_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub path: String,
    pub file_name: String,
    pub alias: Option<String>,
    pub title: Option<String>,
    pub file_kind: String,
    pub workspace_kind: String,
    pub target_url: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreviewDto {
    pub path: String,
    pub file_name: String,
    pub alias: Option<String>,
    pub title: Option<String>,
    pub file_kind: String,
    pub workspace_kind: String,
    pub target_url: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWorkspaceInput {
    pub path: String,
    pub kind: WorkspaceKind,
    pub name: Option<String>,
}
