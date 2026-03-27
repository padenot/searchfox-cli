use crate::client::SearchfoxClient;
use crate::utils::searchfox_url_repo;
use anyhow::Result;
use log::debug;
use scraper::{Html, Selector};

impl SearchfoxClient {
    pub async fn get_file(&self, path: &str) -> Result<String> {
        let url = format!(
            "{}/{}/source/{}",
            self.base_url,
            searchfox_url_repo(&self.repo),
            path
        );

        let cache_key = format!("source:{url}");

        if let Some(entry) = self.cache_get(&cache_key) {
            if entry.is_fresh() {
                debug!("Cache hit (fresh) for: {}", url);
                return Ok(entry.content);
            }

            debug!("Cache stale, revalidating: {}", url);
            match self
                .get_html_with_meta(&url, entry.etag.as_deref(), entry.last_modified.as_deref())
                .await?
            {
                None => {
                    debug!("Cache revalidated (304) for: {}", url);
                    self.cache_set(
                        &cache_key,
                        &entry.content,
                        entry.etag.as_deref(),
                        entry.last_modified.as_deref(),
                    );
                    return Ok(entry.content);
                }
                Some((html, etag, last_modified)) => {
                    let content = parse_source_lines(&html, &url)?;
                    self.cache_set(
                        &cache_key,
                        &content,
                        etag.as_deref(),
                        last_modified.as_deref(),
                    );
                    return Ok(content);
                }
            }
        }

        if self.force_refetch() {
            debug!("Force refetch for: {}", url);
        }

        let (html, etag, last_modified) = self
            .get_html_with_meta(&url, None, None)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Unexpected 304 on cold cache miss"))?;

        let content = parse_source_lines(&html, &url)?;
        self.cache_set(
            &cache_key,
            &content,
            etag.as_deref(),
            last_modified.as_deref(),
        );

        Ok(content)
    }
}

fn parse_source_lines(html: &str, url: &str) -> Result<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("code.source-line").expect("valid selector");
    let lines: Vec<String> = document
        .select(&selector)
        .map(|el| el.text().collect::<String>())
        .collect();
    if lines.is_empty() {
        anyhow::bail!("Could not find file content at {}", url);
    }
    Ok(lines.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const HTML_V1: &str = r#"<html><body>
        <code class="source-line">fn foo() {</code>
        <code class="source-line">    42</code>
        <code class="source-line">}</code>
    </body></html>"#;

    const HTML_V2: &str = r#"<html><body>
        <code class="source-line">fn foo() {</code>
        <code class="source-line">    99</code>
        <code class="source-line">}</code>
    </body></html>"#;

    #[tokio::test]
    async fn cold_miss_fetches_and_returns_parsed_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(HTML_V1)
                    .insert_header("etag", "\"v1\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let content = client.get_file("some/file.js").await.unwrap();

        assert!(content.contains("fn foo()"));
        assert!(!content.contains("<code"));
    }

    #[tokio::test]
    async fn fresh_hit_skips_network() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(HTML_V1)
                    .insert_header("etag", "\"v1\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let first = client.get_file("some/file.js").await.unwrap();
        let second = client.get_file("some/file.js").await.unwrap();

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn stale_hit_304_refreshes_without_reparse() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .and(header("If-None-Match", "\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .expect(1)
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let url = format!("{}/firefox-main/source/some/file.js", server.uri());
        let cache_key = format!("source:{url}");
        client.cache_set_stale(&cache_key, "fn foo() {\n    42\n}\n", Some("\"v1\""), None);

        let content = client.get_file("some/file.js").await.unwrap();
        assert!(content.contains("fn foo()"));
    }

    #[tokio::test]
    async fn server_error_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        assert!(client.get_file("some/file.js").await.is_err());
    }

    #[tokio::test]
    async fn html_without_source_lines_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body>no lines here</body></html>"),
            )
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        assert!(client.get_file("some/file.js").await.is_err());
    }

    #[tokio::test]
    async fn stale_without_etag_does_unconditional_get() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(HTML_V2)
                    .insert_header("etag", "\"v2\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let url = format!("{}/firefox-main/source/some/file.js", server.uri());
        let cache_key = format!("source:{url}");
        client.cache_set_stale(&cache_key, "fn foo() {\n    42\n}\n", None, None);

        let content = client.get_file("some/file.js").await.unwrap();
        assert!(content.contains("99"));
    }

    #[tokio::test]
    async fn stale_hit_200_fetches_and_reparses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/firefox-main/source/some/file.js"))
            .and(header("If-None-Match", "\"v1\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(HTML_V2)
                    .insert_header("etag", "\"v2\""),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = SearchfoxClient::new_for_test("mozilla-central".into(), server.uri()).unwrap();
        let url = format!("{}/firefox-main/source/some/file.js", server.uri());
        let cache_key = format!("source:{url}");
        client.cache_set_stale(&cache_key, "fn foo() {\n    42\n}\n", Some("\"v1\""), None);

        let content = client.get_file("some/file.js").await.unwrap();
        assert!(content.contains("99"));
        assert!(!content.contains("42"));
    }
}
