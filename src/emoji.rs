use std::error::Error;

use serde::{Deserialize, Serialize};

use crate::SlackClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct Emoji {
    name: String,
    url: String,
    #[serde(rename = "user_display_name")]
    added_by: String,
    alias_for: String,
    // TODO: would be nice to record when an emoji was created
}

#[derive(Debug, Serialize, Deserialize)]
struct EmojiResponse {
    #[serde(rename = "emoji")]
    emojis: Vec<Emoji>,
}

pub async fn fetch_slack_custom_emojis(client: &SlackClient) -> Result<(), Box <dyn Error>> {
    let url = client.generate_url("emoji.adminList");
    let response: EmojiResponse = client.client.post(&url)
        .form(&[("token", &client.token)])
        .send()
        .await?
        .json()
        .await?;
    println!("{:#?}", response);
    Ok(())
}