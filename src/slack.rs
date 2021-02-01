#[derive(Debug)]
pub struct SlackClient {
    pub client: reqwest::Client,
    pub token: String,
    pub base_url: String,
}

impl SlackClient {
    pub fn new<S: Into<String>, T: AsRef<str>>(token: S, workspace: T) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: token.into(),
            base_url: format!("https://{}.slack.com/api", workspace.as_ref()),
        }
    }

    pub fn generate_url<T: AsRef<str>>(&self, endpoint: T) -> String {
        format!("{}/{}", self.base_url, endpoint.as_ref())
    }
}
