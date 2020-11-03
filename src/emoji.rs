use std::error::Error;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Emoji {
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

pub async fn fetch_slack_custom_emojis(token: &str, workspace: &str) -> Result<(), Box <dyn Error>> {
    let url = format!("https://{}.slack.com/api/emoji.adminList", workspace);
    let client = reqwest::Client::new();
    let response: EmojiResponse = client.post(&url)
        .form(&[("token", token)])
        .send()
        .await?
        .json()
        .await?;
    println!("{:#?}", response);
    Ok(())
}