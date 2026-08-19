use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLibraryScope {
    Project,
    Session,
}

impl std::fmt::Display for SessionLibraryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Project => write!(f, "project"),
            Self::Session => write!(f, "session"),
        }
    }
}

impl std::str::FromStr for SessionLibraryScope {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "project" => Ok(Self::Project),
            "session" => Ok(Self::Session),
            _ => anyhow::bail!("invalid session library scope {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLibraryItemKind {
    Text,
    Image,
    File,
}

impl std::fmt::Display for SessionLibraryItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Image => write!(f, "image"),
            Self::File => write!(f, "file"),
        }
    }
}

impl std::str::FromStr for SessionLibraryItemKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "file" => Ok(Self::File),
            _ => anyhow::bail!("invalid session library item kind {value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLibraryItem {
    pub id: String,
    pub scope: SessionLibraryScope,
    pub name: String,
    pub kind: SessionLibraryItemKind,
    pub mime_type: String,
    pub size_bytes: usize,
    pub text_content: Option<String>,
    pub image_data: Option<String>,
    pub file_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewSessionLibraryContent {
    Text(String),
    Image { data: String, mime_type: String },
    File { path: String, mime_type: String },
}
