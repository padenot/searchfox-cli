use crate::client::SearchfoxClient;
use crate::utils::searchfox_url_repo;
use anyhow::Result;
use scraper::{Html, Selector};

impl SearchfoxClient {
    pub async fn get_file(&self, path: &str) -> Result<String> {
        let url = format!(
            "https://searchfox.org/{}/source/{}",
            searchfox_url_repo(&self.repo),
            path
        );
        let html = self.get_html(&url).await?;
        let document = Html::parse_document(&html);
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
}
