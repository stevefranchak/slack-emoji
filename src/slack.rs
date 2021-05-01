use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::StreamExt;
use log::{info, trace};
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use serde::Deserialize;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

use crate::archive::EmojiFile;
use crate::emoji::Emoji;

#[derive(Debug)]
pub struct SlackClient {
    pub client: Client,
    pub token: String,
    pub base_url: String,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    error: Option<String>,
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct PagingInfo {
    pages: u16,
}

#[derive(Debug, Deserialize)]
struct EmojiResponse {
    #[serde(rename = "emoji")]
    emojis: Vec<Emoji>,
    paging: PagingInfo,
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

    // TODO - add retry logic if rate limited
    pub async fn fetch_custom_emoji_page(
        &self,
        curr_page: u16,
    ) -> Result<(Vec<Emoji>, u16), Box<dyn Error>> {
        let response: EmojiResponse = self
            .client
            .post(&self.generate_url("emoji.adminList"))
            .form(&[
                ("token", &self.token),
                ("count", &"100".into()),
                ("page", &curr_page.to_string()),
            ])
            .send()
            .await?
            .json()
            .await?;

        Ok((response.emojis, response.paging.pages))
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
        let result = loop {
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
                        "Could not successfully upload emoji within 3 tries, skipping: {:?}",
                        emoji_file
                    ));
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

            break Ok(response.json::<StatusResponse>().await?);
        };

        // Trying to help avoid consistently hitting a rate limit at a certain point
        sleep(Duration::from_secs(1)).await;

        match result {
            Ok(response) => {
                if let Some(error_msg) = response.error {
                    Err(format!(
                        "Failed to upload emoji {} for reason: {}",
                        emoji_file.emoji.name, error_msg
                    )
                    .into())
                } else {
                    info!("Uploaded emoji: {:?}", emoji_file);
                    Ok(())
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn add_alias<T: AsRef<str>>(
        &self,
        name: T,
        alias_for: T,
    ) -> Result<(), Box<dyn Error>> {
        let mut try_count: u8 = 0;
        let result = loop {
            // form needs to be recreated on each iteration of the loop since RequestBuilder moves it
            let form = Form::new()
                .part("mode", Part::text("alias"))
                // clones are needed here because the values passed to reqwest::multipart::Part's text and file_name methods
                // are bound by Into<Cow<'static, str>>, so any references passed in would need to have a 'static lifetime.
                .part("name", Part::text(name.as_ref().to_string()))
                .part("alias_for", Part::text(alias_for.as_ref().to_string()))
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
                        "Could not successfully add alias '{}' for '{}' within 3 tries, skipping",
                        name.as_ref(),
                        alias_for.as_ref()
                    ));
                };
                try_count += 1;
                // TODO: better error handling / maybe a better way to go about this?
                let wait_time_s: u64 = wait_time_s.to_str()?.parse()?;
                trace!(
                    "Hit rate-limit on emoji.add for adding alias '{}' for '{}'; retrying in {} seconds",
                    name.as_ref(), alias_for.as_ref(),
                    wait_time_s
                );
                sleep(Duration::from_secs(wait_time_s)).await;
                continue;
            }

            break Ok(response.json::<StatusResponse>().await?);
        };

        // Trying to help avoid consistently hitting a rate limit at a certain point
        sleep(Duration::from_secs(1)).await;

        match result {
            Ok(response) => {
                if let Some(error_msg) = response.error {
                    Err(format!(
                        "Failed to add alias '{}' for '{}' for reason: {}",
                        name.as_ref(),
                        alias_for.as_ref(),
                        error_msg
                    )
                    .into())
                } else {
                    info!(
                        "Added alias '{}' for '{}'",
                        name.as_ref(),
                        alias_for.as_ref()
                    );
                    Ok(())
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_emoji_response_from_slack_api() {
        let emoji_response_json = r#"
            {
                "ok": true,
                "emoji": [
                    {
                        "name": "-1000",
                        "is_alias": 0,
                        "alias_for": "",
                        "url": "https://emoji.slack-edge.com/T03C6ES54/-1000/test1.png",
                        "created": 1595443479,
                        "team_id": "T12345",
                        "user_id": "U12345",
                        "user_display_name": "Jimmy Dean",
                        "avatar_hash": "eaadc23dd547",
                        "can_delete": true,
                        "is_bad": false,
                        "synonyms": []
                    },
                    {
                        "name": "1000",
                        "is_alias": 1,
                        "alias_for": "-1000",
                        "url": "https://emoji.slack-edge.com/T03C6ES54/1000/test2.png",
                        "created": 1595443506,
                        "team_id": "T12345",
                        "user_id": "U12345",
                        "user_display_name": "SPOONBEARD",
                        "avatar_hash": "eaadc23dd547",
                        "can_delete": false,
                        "is_bad": false,
                        "synonyms": [
                            "1000",
                            "-1000"
                        ]
                    }
                ],
                "disabled_emoji": [],
                "custom_emoji_total_count": 915,
                "paging": {
                    "count": 2,
                    "total": 915,
                    "page": 1,
                    "pages": 458
                }
            }
        "#;

        let parsed_response: EmojiResponse = serde_json::from_str(emoji_response_json).unwrap();

        assert_eq!(parsed_response.emojis.len(), 2);
        assert_eq!(parsed_response.paging.pages, 458);

        assert_eq!(parsed_response.emojis[0].name, "-1000");
        assert_eq!(parsed_response.emojis[0].added_by, "Jimmy Dean");
        assert_eq!(parsed_response.emojis[0].alias_for, "");
        assert_eq!(
            parsed_response.emojis[0].created,
            "2020-07-22T18:44:39Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            parsed_response.emojis[0].url,
            "https://emoji.slack-edge.com/T03C6ES54/-1000/test1.png"
        );

        assert_eq!(parsed_response.emojis[1].name, "1000");
        assert_eq!(parsed_response.emojis[1].added_by, "SPOONBEARD");
        assert_eq!(parsed_response.emojis[1].alias_for, "-1000");
        assert_eq!(
            parsed_response.emojis[1].created,
            "2020-07-22T18:45:06Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            parsed_response.emojis[1].url,
            "https://emoji.slack-edge.com/T03C6ES54/1000/test2.png"
        );

        let encoded_as_string = serde_json::to_string(&parsed_response.emojis[1]).unwrap();
        assert_eq!(
            encoded_as_string,
            r#"{"name":"1000","url":"https://emoji.slack-edge.com/T03C6ES54/1000/test2.png","added_by":"SPOONBEARD","alias_for":"-1000","created":"2020-07-22T18:45:06Z"}"#
        );

        // Quick test that we can deserialize the just-serialized string to test deserialize_with = "from_ts_or_string"
        let parsed_emoji: Emoji = serde_json::from_str(&encoded_as_string).unwrap();
        assert_eq!(
            parsed_emoji.created,
            "2020-07-22T18:45:06Z".parse::<DateTime<Utc>>().unwrap()
        );
    }
}
