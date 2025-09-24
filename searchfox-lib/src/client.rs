use crate::types::{RequestLog, ResponseLog};
use anyhow::Result;
use log::debug;
use reqwest::{Client, Url};
use std::time::{Duration, Instant};

pub struct SearchfoxClient {
    client: Client,
    pub repo: String,
    pub log_requests: bool,
    request_counter: std::sync::atomic::AtomicUsize,
}

impl SearchfoxClient {
    pub fn new(repo: String, log_requests: bool) -> Result<Self> {
        let client = Self::create_tls13_client()?;
        Ok(Self {
            client,
            repo,
            log_requests,
            request_counter: std::sync::atomic::AtomicUsize::new(0),
        })
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

    pub fn client(&self) -> &Client {
        &self.client
    }
}
