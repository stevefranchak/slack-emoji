#[derive(Debug)]
pub struct SlackClient {
    pub client: reqwest::Client,
    pub token: String,
    pub base_url: String,
}

impl SlackClient {
    pub fn new(token: String, workspace: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            base_url: format!("https://{}.slack.com/api", workspace),
        }
    }

    pub fn generate_url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.base_url, endpoint)
    }
}