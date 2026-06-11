use crate::types::{RequestLog, ResponseLog};
use anyhow::Result;
use log::debug;
use reqwest::{Client, Url};
use std::time::{Duration, Instant};

pub struct SearchfoxClient {
    client: Client,
    pub repo: String,
    pub log_requests: bool,
    pub(crate) base_url: String,
    request_counter: std::sync::atomic::AtomicUsize,
    cache: Option<std::sync::Mutex<rusqlite::Connection>>,
    cache_enabled: bool,
    force_refetch: bool,
}

impl SearchfoxClient {
    pub fn new(repo: String, log_requests: bool) -> Result<Self> {
        let client = Self::create_tls13_client()?;
        let cache = crate::cache::open().map(|conn| {
            crate::cache::prune(&conn);
            std::sync::Mutex::new(conn)
        });
        Ok(Self {
            client,
            repo,
            log_requests,
            base_url: "https://searchfox.org".to_string(),
            request_counter: std::sync::atomic::AtomicUsize::new(0),
            cache,
            cache_enabled: true,
            force_refetch: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(repo: String, base_url: String) -> Result<Self> {
        let client = Self::create_tls13_client()?;
        let conn = crate::cache::open_in_memory()?;
        Ok(Self {
            client,
            repo,
            log_requests: false,
            base_url,
            request_counter: std::sync::atomic::AtomicUsize::new(0),
            cache: Some(std::sync::Mutex::new(conn)),
            cache_enabled: true,
            force_refetch: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn cache_set_stale(
        &self,
        key: &str,
        content: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) {
        if let Some(ref m) = self.cache {
            if let Ok(c) = m.lock() {
                crate::cache::set_with_timestamp(&c, key, content, etag, last_modified, 0);
            }
        }
    }

    fn create_tls13_client() -> Result<Client> {
        Client::builder()
            .user_agent(Self::get_user_agent())
            .use_rustls_tls()
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            .max_tls_version(reqwest::tls::Version::TLS_1_3)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build TLS client with rustls: {}", e))
    }

    fn get_user_agent() -> String {
        let magic_word = std::env::var("SEARCHFOX_MAGIC_WORD")
            .unwrap_or_else(|_| "sésame ouvre toi".to_string());
        format!("searchfox-cli/{} ({})", crate::VERSION, magic_word)
    }

    pub fn log_request_start(&self, method: &str, url: &str) -> Option<RequestLog> {
        if !self.log_requests {
            return None;
        }

        let request_id = self
            .request_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;

        let log = RequestLog {
            url: url.to_string(),
            method: method.to_string(),
            start_time: Instant::now(),
            request_id,
        };

        eprintln!(
            "[REQ-{}] {} {} - START",
            log.request_id, log.method, log.url
        );
        Some(log)
    }

    pub fn log_request_end(&self, request_log: RequestLog, status: u16, size_bytes: usize) {
        let duration = request_log.start_time.elapsed();

        let response_log = ResponseLog {
            request_id: request_log.request_id,
            status,
            size_bytes,
            duration,
        };

        eprintln!(
            "[REQ-{}] {} {} - END ({}ms, {} bytes, HTTP {})",
            response_log.request_id,
            request_log.method,
            request_log.url,
            duration.as_millis(),
            size_bytes,
            status
        );
    }

    pub async fn ping(&self) -> Result<Duration> {
        if !self.log_requests {
            return Ok(Duration::from_millis(0));
        }

        eprintln!(
            "[PING] Testing network latency to searchfox.org (ICMP ping disabled, using HTTP HEAD)..."
        );

        let ping_url = "https://searchfox.org/";
        let start = Instant::now();

        let response = self
            .client
            .head(ping_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        let latency = start.elapsed();

        eprintln!(
            "[PING] HTTP HEAD latency: {}ms (HTTP {})",
            latency.as_millis(),
            response.status()
        );
        eprintln!(
            "[PING] Note: This includes minimal server processing time, not just network latency"
        );

        Ok(latency)
    }

    pub async fn get(&self, url: Url) -> Result<reqwest::Response> {
        let request_log = self.log_request_start("GET", url.as_ref());
        let response = self
            .client
            .get(url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;

        if let Some(req_log) = request_log {
            self.log_request_end(req_log, response.status().as_u16(), 0);
        }

        Ok(response)
    }

    pub async fn get_raw(&self, url: &str) -> Result<String> {
        let request_log = self.log_request_start("GET", url);
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            if let Some(req_log) = request_log {
                self.log_request_end(req_log, response.status().as_u16(), 0);
            }
            anyhow::bail!("Request failed: {}", response.status());
        }

        let text = response.text().await?;
        let size = text.len();

        if let Some(req_log) = request_log {
            self.log_request_end(req_log, 200, size);
        }

        Ok(text)
    }

    pub async fn get_final_url(&self, url: &str) -> Result<String> {
        let url = reqwest::Url::parse(url).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
        let response = self.client.head(url).send().await?;
        Ok(response.url().as_str().to_string())
    }

    pub async fn get_html(&self, url: &str) -> Result<String> {
        debug!("Fetching HTML from: {}", url);

        let response = self
            .client
            .get(url)
            .header("Accept", "text/html")
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Request failed: {}", response.status());
        }

        Ok(response.text().await?)
    }

    pub async fn get_html_with_meta(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<Option<(String, Option<String>, Option<String>)>> {
        debug!("Fetching HTML from: {}", url);

        let mut request = self.client.get(url).header("Accept", "text/html");
        if let Some(etag) = etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(lm) = last_modified {
            request = request.header("If-Modified-Since", lm);
        }

        let response = request.send().await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        if !status.is_success() {
            anyhow::bail!("Request failed: {}", status);
        }

        let etag = response
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let last_modified = response
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let text = response.text().await?;
        Ok(Some((text, etag, last_modified)))
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
    }

    pub fn set_force_refetch(&mut self, force_refetch: bool) {
        self.force_refetch = force_refetch;
    }

    pub(crate) fn force_refetch(&self) -> bool {
        self.force_refetch
    }

    pub(crate) fn cache_get(&self, url: &str) -> Option<crate::cache::CacheEntry> {
        if !self.cache_enabled || self.force_refetch {
            return None;
        }
        self.cache
            .as_ref()?
            .lock()
            .ok()
            .and_then(|c| crate::cache::get(&c, url))
    }

    pub(crate) fn cache_set(
        &self,
        url: &str,
        content: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) {
        if !self.cache_enabled {
            return;
        }
        if let Some(ref m) = self.cache {
            if let Ok(c) = m.lock() {
                crate::cache::set(&c, url, content, etag, last_modified);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cache_disables_reads_and_writes() {
        let mut client =
            SearchfoxClient::new_for_test("mozilla-central".into(), "https://example.com".into())
                .unwrap();

        client.cache_set("source:https://example.com/a", "v1", None, None);
        assert!(client.cache_get("source:https://example.com/a").is_some());

        client.set_cache_enabled(false);
        assert!(client.cache_get("source:https://example.com/a").is_none());

        client.cache_set("source:https://example.com/b", "v2", None, None);
        client.set_cache_enabled(true);
        assert!(client.cache_get("source:https://example.com/b").is_none());
    }

    #[test]
    fn force_refetch_bypasses_reads_but_keeps_writes() {
        let mut client =
            SearchfoxClient::new_for_test("mozilla-central".into(), "https://example.com".into())
                .unwrap();

        client.cache_set("source:https://example.com/a", "v1", None, None);
        assert!(client.cache_get("source:https://example.com/a").is_some());

        client.set_force_refetch(true);
        assert!(client.cache_get("source:https://example.com/a").is_none());

        client.cache_set("source:https://example.com/b", "v2", None, None);
        client.set_force_refetch(false);
        assert_eq!(
            client
                .cache_get("source:https://example.com/b")
                .unwrap()
                .content,
            "v2"
        );
    }
}
