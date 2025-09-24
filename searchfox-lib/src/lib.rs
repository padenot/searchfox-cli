pub mod client;
pub mod types;
pub mod search;
pub mod call_graph;
pub mod definition;
pub mod file_reader;
pub mod utils;

pub use client::SearchfoxClient;
pub use types::*;
pub use search::SearchOptions;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");