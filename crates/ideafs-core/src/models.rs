use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetaData {
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Serialize)]
pub struct IdeaFile {
    pub filepath: String,
    pub metadata: MetaData,
}

#[derive(Serialize)]
pub struct IdeaFolder {
    pub name: String,
    pub status: String,
    pub files: Vec<IdeaFile>,
}

#[derive(Serialize)]
pub struct TreeData {
    pub folders: Vec<IdeaFolder>,
}

#[derive(Serialize)]
pub struct FileData {
    pub filepath: String,
    pub metadata: MetaData,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct CliIdea {
    pub filepath: String,
    pub metadata: MetaData,
}