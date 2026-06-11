pub mod blame;
pub mod cache;
pub mod call_graph;
pub mod can_gc;
pub mod client;
pub mod definition;
pub mod field_layout;
pub mod file_reader;
pub mod nesting;
pub mod search;
pub mod types;
pub mod utils;

pub use blame::parse_commit_header;
pub use client::SearchfoxClient;
pub use search::{CategoryFilter, Lang, SearchOptions};
pub use types::*;
pub use utils::searchfox_url_repo;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub enum SearchfoxErrorKind {
    Network,
    Request,
    Other,
}

pub fn classify_error(e: &anyhow::Error) -> SearchfoxErrorKind {
    if let Some(re) = e.downcast_ref::<reqwest::Error>() {
        if let Some(status) = re.status() {
            if status.is_server_error() {
                return SearchfoxErrorKind::Network;
            }
            if status.is_client_error() {
                return SearchfoxErrorKind::Request;
            }
        }
        return SearchfoxErrorKind::Network;
    }

    let msg = e.to_string();
    if let Some(rest) = msg.strip_prefix("Request failed: ") {
        if let Some(code_str) = rest.split_whitespace().next() {
            if let Ok(code) = code_str.parse::<u16>() {
                if code >= 500 {
                    return SearchfoxErrorKind::Network;
                }
                if code >= 400 {
                    return SearchfoxErrorKind::Request;
                }
            }
        }
    }

    if msg.contains("Could not find file content") {
        return SearchfoxErrorKind::Request;
    }

    SearchfoxErrorKind::Other
}
