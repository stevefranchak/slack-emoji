use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use futures::stream::StreamExt;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

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

    pub async fn download<P: AsRef<Path>, T: AsRef<str>>(
        &self,
        download_url: T,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let mut emoji_file = File::create(path).await?;
        let mut stream = self
            .client
            .get(download_url.as_ref())
            .send()
            .await?
            .bytes_stream();

        while let Some(Ok(chunk)) = stream.next().await {
            emoji_file.write_all(&chunk).await?;
        }
        emoji_file.flush().await?;

        Ok(())
    }
}
