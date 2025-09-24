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