pub mod auth;
pub mod bbs;
pub mod info_service;

pub const REMOTE_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.7727.102 Safari/537.36";

pub fn remote_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder().user_agent(REMOTE_BROWSER_USER_AGENT)
}

pub fn remote_client() -> Result<reqwest::Client, reqwest::Error> {
    remote_client_builder().build()
}
