use std::error::Error;
use std::rc::Rc;

use async_stream::try_stream;
use chrono::prelude::*;
use chrono::serde::ts_seconds::deserialize as from_ts;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_compat_02::FutureExt;

use crate::slack::SlackClient;

// TODO: Might not be able to deserialize from output written to disk?
#[derive(Debug, Serialize, Deserialize)]
pub struct Emoji {
    pub name: String,
    pub url: String,
    #[serde(rename(deserialize = "user_display_name"))]
    pub added_by: String,
    pub alias_for: String,
    #[serde(deserialize_with = "from_ts")]
    pub created: DateTime<Utc>,
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

pub struct EmojiPaginator {
    client: Rc<SlackClient>,
    per_page: u16,
}

impl EmojiPaginator {
    pub fn new(client: Rc<SlackClient>, per_page: u16) -> Self {
        Self {client, per_page}
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Emoji, Box<dyn Error>>> {
        try_stream! {
            let mut curr_page: u16 = 1;
            let mut num_pages: Option<u16> = None;
            loop {
                if let Some(num_pages) = num_pages {
                    if curr_page > num_pages {
                        break;
                    }
                }
                let response = self.fetch_slack_custom_emojis(curr_page).await?;
                if num_pages.is_none() {
                    num_pages = Some(response.paging.pages);
                }
                for emoji in response.emojis {
                    yield emoji;
                }
                curr_page += 1;
            }
        }
    }

    async fn fetch_slack_custom_emojis(&self, curr_page: u16) -> Result<EmojiResponse, Box <dyn Error>> {
        let url = self.client.generate_url("emoji.adminList");
        let response: EmojiResponse = self.client.client.post(&url)
            .form(&[
                ("token", &self.client.token),
                ("count", &self.per_page.to_string()),
                ("page", &curr_page.to_string()),
            ])
            .send()
            .compat()  // hyper requires tokio 0.2 runtime, waiting on hyper 0.14 (see reqwest #1060)
            .await?
            .json()
            .await?;
        Ok(response)
    }
}