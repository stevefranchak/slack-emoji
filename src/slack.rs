use std::error::Error;
use std::path::Path;
use std::time::Duration;

use futures::stream::StreamExt;
use log::trace;
use reqwest::Client;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

use crate::emoji::EmojiExistenceKind;

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

    // TODO: deprecated, but keeping for now as a reference
    pub async fn _does_emoji_exist<S: Into<String>>(
        &self,
        name: S,
    ) -> Result<EmojiExistenceKind, Box<dyn Error>> {
        #[derive(Debug, Deserialize)]
        struct EmojiGetInfoResponse {
            error: Option<String>,
            alias_for: Option<String>,
        }

        use EmojiExistenceKind::*;

        let mut loop_counter: u8 = 0;
        let emoji_name: String = name.into();

        let result = loop {
            let response = self
                .client
                .post(&self.generate_url("emoji.getInfo"))
                .form(&[("token", &self.token), ("name", &emoji_name)])
                .send()
                .await?;

            // TODO: this all needs to be abstracted better... this is a mess
            if let Some(wait_time_s) = response.headers().get("retry-after") {
                if loop_counter == 3 {
                    break Err(
                        "could not successfully get an emoji.getInfo response within 3 tries",
                    );
                }
                loop_counter += 1;
                // TODO: better error handling / maybe a better way to go about this?
                let wait_time_s: u64 = wait_time_s.to_str()?.parse()?;
                trace!(
                    "Hit rate-limit on emoji.getInfo; retrying in {} seconds",
                    wait_time_s
                );
                sleep(Duration::from_secs(wait_time_s)).await;
                continue;
            }

            break Ok(response.json::<EmojiGetInfoResponse>().await?);
        };

        match result {
            Ok(response) => {
                if let Some(alias_for) = response.alias_for {
                    Ok(EmojiExistsAsAliasFor(alias_for))
                } else if let Some(error) = response.error {
                    if error != "emoji_not_found" {
                        Err(format!("received response error: {}", error).into())
                    } else {
                        Ok(EmojiDoesNotExist)
                    }
                } else {
                    Ok(EmojiExists)
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}
