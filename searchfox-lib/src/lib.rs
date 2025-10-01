pub mod call_graph;
pub mod client;
pub mod definition;
pub mod file_reader;
pub mod search;
pub mod types;
pub mod utils;

pub use client::SearchfoxClient;
pub use search::{CategoryFilter, SearchOptions};
pub use types::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
