use crate::client::SearchfoxClient;
use crate::utils::get_github_raw_url;
use anyhow::Result;
use log::error;

impl SearchfoxClient {
    pub async fn get_file(&self, path: &str) -> Result<String> {
        let github_url = get_github_raw_url(&self.repo, path);

        match self.get_raw(&github_url).await {
            Ok(text) => Ok(text),
            Err(e) => {
                error!(
                    "GitHub fetch failed ({}). You can try viewing it at:\nhttps://searchfox.org/{}/source/{}",
                    e, self.repo, path
                );
                anyhow::bail!("Could not fetch file from GitHub");
            }
        }
    }
}