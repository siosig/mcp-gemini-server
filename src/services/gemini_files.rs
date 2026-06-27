//! Gemini Files API operations (upload / list / status / delete).
//!
//! Implemented as additional inherent methods on [`GeminiClient`]. The uploads use a
//! manually-assembled `multipart/related` body (matching the REST contract); reading
//! the file bytes locally sidesteps the SDK's non-ASCII-filename header limitation.

use serde::{Deserialize, Serialize};

use crate::services::gemini_client::GeminiClient;
use crate::utils::errors::GeminiError;
use crate::utils::telemetry::with_timeout;

const API_BASE: &str = "https://generativelanguage.googleapis.com";

/// A file entry as returned to the `gemini_manage_files` tool.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<String>,
    /// Serialized as `null` when absent (mirrors the original `?? null`).
    pub expiration_time: Option<String>,
}

/// Wire representation (camelCase) of a Files API resource.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct FileWire {
    name: Option<String>,
    display_name: Option<String>,
    mime_type: Option<String>,
    state: Option<String>,
    uri: Option<String>,
    size_bytes: Option<String>,
    expiration_time: Option<String>,
}

impl From<FileWire> for FileEntry {
    fn from(f: FileWire) -> Self {
        FileEntry {
            name: f.name,
            display_name: f.display_name,
            mime_type: f.mime_type,
            state: f.state.unwrap_or_else(|| "UNKNOWN".to_string()),
            uri: f.uri,
            size_bytes: f.size_bytes,
            expiration_time: f.expiration_time,
        }
    }
}

#[derive(Deserialize)]
struct UploadResponse {
    file: FileWire,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ListResponse {
    files: Vec<FileWire>,
    next_page_token: Option<String>,
}

impl GeminiClient {
    fn timeout_ms(&self) -> u64 {
        self.config.timeout_ms
    }

    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, GeminiError> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp)
        } else {
            let message = resp.text().await.unwrap_or_default();
            Err(GeminiError::Http {
                status: status.as_u16(),
                message,
            })
        }
    }

    /// Upload a local file via `multipart/related`.
    pub async fn upload_file(
        &self,
        file_path: &str,
        display_name: Option<&str>,
    ) -> Result<FileEntry, GeminiError> {
        let timeout = self.timeout_ms().saturating_mul(2);
        with_timeout(self.upload_file_inner(file_path, display_name), timeout).await
    }

    async fn upload_file_inner(
        &self,
        file_path: &str,
        display_name: Option<&str>,
    ) -> Result<FileEntry, GeminiError> {
        let bytes = tokio::fs::read(file_path).await?;
        let mime = mime_guess::from_path(file_path)
            .first_or_octet_stream()
            .to_string();
        let basename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("upload");
        let display = display_name.unwrap_or(basename);

        let boundary = format!("mcp-gemini-{}", uuid::Uuid::new_v4().simple());
        let metadata = serde_json::json!({ "file": { "displayName": display } });

        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(metadata.to_string().as_bytes());
        body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
        body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
        body.extend_from_slice(&bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let url = format!("{API_BASE}/upload/v1beta/files?uploadType=multipart");
        let resp = self
            .http
            .post(&url)
            .header("x-goog-api-key", self.api_key())
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/related; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await?;

        let resp = Self::check_status(resp).await?;
        let parsed: UploadResponse = resp.json().await?;
        Ok(parsed.file.into())
    }

    /// List all files, following pagination.
    pub async fn list_files(&self) -> Result<Vec<FileEntry>, GeminiError> {
        with_timeout(self.list_files_inner(), self.timeout_ms()).await
    }

    async fn list_files_inner(&self) -> Result<Vec<FileEntry>, GeminiError> {
        let mut entries = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .http
                .get(format!("{API_BASE}/v1beta/files"))
                .header("x-goog-api-key", self.api_key());
            if let Some(token) = &page_token {
                req = req.query(&[("pageToken", token)]);
            }

            let resp = Self::check_status(req.send().await?).await?;
            let page: ListResponse = resp.json().await?;
            entries.extend(page.files.into_iter().map(FileEntry::from));

            match page.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(entries)
    }

    /// Fetch a single file's status.
    pub async fn get_file_status(&self, file_name: &str) -> Result<FileEntry, GeminiError> {
        with_timeout(self.get_file_status_inner(file_name), self.timeout_ms()).await
    }

    async fn get_file_status_inner(&self, file_name: &str) -> Result<FileEntry, GeminiError> {
        let url = format!("{API_BASE}/v1beta/{file_name}");
        let resp = self
            .http
            .get(&url)
            .header("x-goog-api-key", self.api_key())
            .send()
            .await?;
        let resp = Self::check_status(resp).await?;
        let wire: FileWire = resp.json().await?;
        Ok(wire.into())
    }

    /// Delete a file by name (`files/abc123`).
    pub async fn delete_file(&self, file_name: &str) -> Result<(), GeminiError> {
        with_timeout(self.delete_file_inner(file_name), self.timeout_ms()).await
    }

    async fn delete_file_inner(&self, file_name: &str) -> Result<(), GeminiError> {
        let url = format!("{API_BASE}/v1beta/{file_name}");
        let resp = self
            .http
            .delete(&url)
            .header("x-goog-api-key", self.api_key())
            .send()
            .await?;
        Self::check_status(resp).await?;
        Ok(())
    }
}
