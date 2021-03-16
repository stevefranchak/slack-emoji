use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::StreamExt;
use log::{info, trace};
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

use crate::archive::EmojiFile;

#[derive(Debug)]
pub struct SlackClient {
    pub client: Client,
    pub token: String,
    pub base_url: String,
}

impl SlackClient {
    pub fn new<S: Into<String>, T: AsRef<str>>(token: S, workspace: T) -> Self {
        Self {
            client: Client::new(),
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

    pub async fn upload(
        &self,
        emoji_file: &EmojiFile,
        emoji_filepath: PathBuf,
    ) -> Result<(), Box<dyn Error>> {
        let mut try_count: u8 = 0;
        loop {
            // form needs to be recreated on each iteration of the loop since RequestBuilder moves it
            let form = Form::new()
                .part("mode", Part::text("data"))
                // clones are needed here because the values passed to reqwest::multipart::Part's text and file_name methods
                // are bound by Into<Cow<'static, str>>, so any references passed in would need to have a 'static lifetime.
                .part("name", Part::text(emoji_file.emoji.name.clone()))
                .part(
                    "image",
                    Part::bytes(fs::read(emoji_filepath.clone()).await?)
                        .file_name(emoji_file.filename.clone()),
                )
                .part("token", Part::text(self.token.clone()));

            let response = self
                .client
                .post(&self.generate_url("emoji.add"))
                .multipart(form)
                .send()
                .await?;

            // TODO: if multiple Slack requests rely on handling rate-limiting, could this be better abstracted with a macro?
            if let Some(wait_time_s) = response.headers().get("retry-after") {
                if try_count == 3 {
                    break Err(format!(
                        "could not successfully upload emoji within 3 tries, skipping: {:?}",
                        emoji_file
                    )
                    .into());
                };
                try_count += 1;
                // TODO: better error handling / maybe a better way to go about this?
                let wait_time_s: u64 = wait_time_s.to_str()?.parse()?;
                trace!(
                    "Hit rate-limit on emoji.add for emoji {}; retrying in {} seconds",
                    emoji_file.emoji.name,
                    wait_time_s
                );
                sleep(Duration::from_secs(wait_time_s)).await;
                continue;
            }

            info!("Uploaded emoji: {:?}", emoji_file);

            // Trying to help with consistently hitting a rate limit at a certain point
            sleep(Duration::from_secs(1)).await;
            break Ok(());
        }
    }
}
