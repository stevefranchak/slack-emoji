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
            .await?
            .json()
            .await?;
        Ok(response)
    }
}