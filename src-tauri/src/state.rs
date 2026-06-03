use serde::{Deserialize, Serialize};

/// 单个文件的条目信息（前端使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// 文件名
    pub name: String,
    /// 本地磁盘绝对路径
    pub path: String,
    /// ISO 内路径（如 /README.TXT）
    pub iso_path: String,
    /// 文件大小（字节）
    pub size: u64,
}

/// 构建请求（前端 → Rust）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    pub volume_label: String,
    pub files: Vec<FileEntry>,
    pub output_path: String,
}

/// 进度事件（Rust → 前端）
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub status: String,
    pub current: usize,
    pub total: usize,
    pub filename: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// 模板条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub volume_label: String,
    pub output_path: String,
    pub files: Vec<FileEntry>,
}

/// 全局应用状态
#[derive(Default)]
pub struct AppState {
    pub files: Vec<FileEntry>,
}
