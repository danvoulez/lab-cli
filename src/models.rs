use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedMetadata {
    pub title: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub document_type: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PipelineOutput {
    pub file: String,
    pub source_relative_path: Option<String>,
    pub sha256: String,
    pub canonical_name: String,
    pub normalized_title: String,
    pub status: String,
    pub note: Option<String>,
    pub drive_file_id: Option<String>,
    pub drive_web_view_link: Option<String>,
    pub supabase_object_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DriveFile {
    pub id: String,
    #[serde(rename = "webViewLink")]
    pub web_view_link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DriveListResponse {
    pub files: Option<Vec<DriveFile>>,
}

#[derive(Debug, Deserialize)]
pub struct DriveUploadResponse {
    pub id: String,
    #[serde(rename = "webViewLink")]
    pub web_view_link: Option<String>,
}
