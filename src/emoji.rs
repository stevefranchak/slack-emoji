use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use async_stream::try_stream;
use chrono::prelude::*;
use chrono::serde::ts_seconds::deserialize as from_ts;
use futures::stream::Stream;
use serde::{
    de::{self, IntoDeserializer},
    Deserialize, Deserializer, Serialize,
};

use crate::slack::SlackClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct Emoji {
    pub name: String,
    pub url: String,
    #[serde(alias = "user_display_name")]
    pub added_by: String,
    pub alias_for: String,
    #[serde(deserialize_with = "from_ts_or_string")]
    pub created: DateTime<Utc>,
}

fn from_ts_or_string<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    struct FromTsOrStringVisitor;

    impl<'de> de::Visitor<'de> for FromTsOrStringVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a formatted date and time string, a unix timestamp, or a unix timestamp in seconds")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            from_ts(value.into_deserializer())
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            from_ts(value.into_deserializer())
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            FromStr::from_str(value).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(FromTsOrStringVisitor)
}

#[derive(Debug, Serialize, Deserialize)]
struct PagingInfo {
    count: u16,
    total: u16,
    page: u16,
    pages: u16,
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
        Self { client, per_page }
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

    async fn fetch_slack_custom_emojis(
        &self,
        curr_page: u16,
    ) -> Result<EmojiResponse, Box<dyn Error>> {
        let url = self.client.generate_url("emoji.adminList");
        println!("{:?}", url);
        let response: EmojiResponse = self
            .client
            .client
            .post(&url)
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
