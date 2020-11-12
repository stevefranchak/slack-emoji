use std::error::Error;
use std::rc::Rc;

use async_stream::try_stream;
use futures::stream::Stream;
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
struct PagingInfo {
    count: u16,
    total: u16,
    page: u16,
    pages: u16
}

#[derive(Debug, Serialize, Deserialize)]
struct EmojiResponse {
    #[serde(rename = "emoji")]
    emojis: Vec<Emoji>,
    paging: PagingInfo,
}

pub fn fetch_slack_custom_emojis(client: Rc<SlackClient>) -> impl Stream<Item = Result<Emoji, Box<dyn Error>>> {
    try_stream! {
        let url = client.generate_url("emoji.adminList");
        let response: EmojiResponse = client.client.post(&url)
            .form(&[
                ("token", &client.token),
                ("count", &5.to_string()),
                ("page", &String::from("10")),
            ])
            .send()
            .await?
            .json()
            .await?;
        for emoji in response.emojis {
            yield emoji;
        }
    }
}