use searchfox_lib::{CategoryFilter, SearchOptions, SearchfoxClient};

fn client() -> SearchfoxClient {
    SearchfoxClient::new("mozilla-central".to_string(), false).unwrap()
}

fn default_opts() -> SearchOptions {
    SearchOptions::default()
}

// --- search ---

#[tokio::test]
async fn search_text_returns_results() {
    let results = client()
        .search(&SearchOptions {
            query: Some("AudioStream".to_string()),
            ..default_opts()
        })
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.line.contains("AudioStream")));
}

#[tokio::test]
async fn search_id_returns_results() {
    let results = client()
        .search(&SearchOptions {
            id: Some("AudioStream".to_string()),
            ..default_opts()
        })
        .await
        .unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn search_path_only_returns_files() {
    let results = client()
        .search(&SearchOptions {
            path: Some("AudioStream.h".to_string()),
            ..default_opts()
        })
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().all(|r| r.line_number == 0));
    assert!(results.iter().any(|r| r.path.contains("AudioStream.h")));
}

#[tokio::test]
async fn search_category_filter_excludes_tests() {
    let all = client()
        .search(&SearchOptions {
            query: Some("AudioStream".to_string()),
            ..default_opts()
        })
        .await
        .unwrap();
    let filtered = client()
        .search(&SearchOptions {
            query: Some("AudioStream".to_string()),
            category_filter: CategoryFilter::ExcludeTests,
            ..default_opts()
        })
        .await
        .unwrap();
    assert!(filtered.len() <= all.len());
}

// --- get_file ---

#[tokio::test]
async fn get_file_returns_content() {
    let content = client().get_file("dom/media/AudioStream.h").await.unwrap();
    assert!(content.contains("AudioStream"));
    assert!(content.contains("mozilla"));
    assert!(content.lines().count() > 10);
}

#[tokio::test]
async fn get_file_nonexistent_returns_error() {
    let result = client().get_file("this/does/not/exist.cpp").await;
    assert!(result.is_err());
}

// --- definition ---

#[tokio::test]
async fn find_definition_returns_result() {
    let result = client()
        .find_and_display_definition("AudioContext::CreateGain", None, &default_opts())
        .await
        .unwrap();
    assert!(!result.is_empty());
    assert!(result.contains("CreateGain"));
}

#[tokio::test]
async fn find_definition_unknown_symbol_returns_empty() {
    let result = client()
        .find_and_display_definition("ThisSymbolDoesNotExistXXX", None, &default_opts())
        .await
        .unwrap();
    assert!(result.is_empty());
}

// --- head hash ---

#[tokio::test]
async fn get_head_hash_returns_valid_hash() {
    let hash = client().get_head_hash().await.unwrap();
    assert_eq!(hash.len(), 40);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

// --- call graph ---

#[tokio::test]
async fn calls_from_returns_results() {
    use searchfox_lib::call_graph::CallGraphQuery;
    let query = CallGraphQuery {
        calls_from: Some("mozilla::dom::AudioContext::CreateGain".to_string()),
        calls_to: None,
        calls_between: None,
        depth: 1,
    };
    let result = client().search_call_graph(&query).await.unwrap();
    assert!(
        result.as_object().is_some_and(|o| !o.is_empty())
            || result.as_array().is_some_and(|a| !a.is_empty())
    );
}

// --- build_query unit tests (no network) ---

#[test]
fn build_query_symbol() {
    let opts = SearchOptions {
        symbol: Some("_ZN7mozilla3dom12AudioContextE".to_string()),
        ..default_opts()
    };
    assert_eq!(opts.build_query(), "symbol:_ZN7mozilla3dom12AudioContextE");
}

#[test]
fn build_query_id() {
    let opts = SearchOptions {
        id: Some("AudioContext".to_string()),
        ..default_opts()
    };
    assert_eq!(opts.build_query(), "id:AudioContext");
}

#[test]
fn build_query_text_with_context() {
    let opts = SearchOptions {
        query: Some("AudioStream".to_string()),
        context: Some(3),
        ..default_opts()
    };
    assert_eq!(opts.build_query(), "context:3 text:AudioStream");
}

#[test]
fn build_query_passthrough_prefixed() {
    let opts = SearchOptions {
        query: Some("path:dom/media AudioStream".to_string()),
        ..default_opts()
    };
    assert_eq!(opts.build_query(), "path:dom/media AudioStream");
}

// --- searchfox_url_repo ---

#[test]
fn url_repo_remaps_mozilla_central() {
    assert_eq!(
        searchfox_lib::searchfox_url_repo("mozilla-central"),
        "firefox-main"
    );
    assert_eq!(
        searchfox_lib::searchfox_url_repo("autoland"),
        "firefox-autoland"
    );
    assert_eq!(
        searchfox_lib::searchfox_url_repo("comm-central"),
        "comm-central"
    );
}
