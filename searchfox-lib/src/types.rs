use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Line {
    pub lno: usize,
    pub line: String,
    #[allow(dead_code)]
    pub bounds: Option<Vec<usize>>,
    #[allow(dead_code)]
    pub context: Option<String>,
    #[allow(dead_code)]
    pub contextsym: Option<String>,
    #[serde(rename = "peekRange")]
    #[allow(dead_code)]
    pub peek_range: Option<String>,
    pub upsearch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub path: String,
    pub lines: Vec<Line>,
}

pub type SearchfoxResponse = HashMap<String, serde_json::Value>;

#[derive(Debug)]
pub struct RequestLog {
    pub url: String,
    pub method: String,
    pub start_time: std::time::Instant,
    pub request_id: usize,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ResponseLog {
    pub request_id: usize,
    pub status: u16,
    pub size_bytes: usize,
    pub duration: std::time::Duration,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitInfo {
    pub header: String,
    pub parent: Option<String>,
    pub date: String,
    pub fulldiff: Option<String>,
    pub phab: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlameInfo {
    pub commit_hash: String,
    pub original_path: String,
    pub original_line: usize,
    pub commit_info: Option<CommitInfo>,
}

#[derive(Debug, Clone)]
pub struct ParsedCommitInfo {
    pub bug_number: Option<u64>,
    pub message: String,
    pub author: String,
    pub date: String,
}
