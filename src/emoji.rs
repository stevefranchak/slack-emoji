use std::collections::hash_map::HashMap;
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use async_stream::try_stream;
use chrono::prelude::*;
use chrono::serde::ts_seconds::deserialize as from_ts;
use futures::pin_mut;
use futures::stream::{Stream, StreamExt};
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

pub const DEFAULT_STARTING_PAGE: u16 = 1;
pub const DEFAULT_NUM_EMOJIS_PER_PAGE: u8 = 100;

pub struct EmojiStreamParameters {
    starting_page_number: u16,
    num_emojis_per_page: u8,
    limit_num_pages: Option<u16>,
}

impl Default for EmojiStreamParameters {
    fn default() -> Self {
        Self {
            starting_page_number: DEFAULT_STARTING_PAGE,
            num_emojis_per_page: DEFAULT_NUM_EMOJIS_PER_PAGE,
            limit_num_pages: None,
        }
    }
}

impl EmojiStreamParameters {
    pub fn new(
        starting_page_number: u16,
        num_emojis_per_page: u8,
        limit_num_pages: Option<u16>,
    ) -> Self {
        Self {
            starting_page_number,
            num_emojis_per_page,
            limit_num_pages,
        }
    }
}

#[derive(Debug)]
pub enum EmojiExistenceKind {
    Exists,
    ExistsAsAliasFor(String),
    DoesNotExist,
}

#[derive(Debug)]
pub struct EmojiCollection(HashMap<String, Emoji>);

impl EmojiCollection {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&mut self, emoji: Emoji) -> Option<Emoji> {
        self.0.insert(emoji.name.clone(), emoji)
    }

    pub fn get_existence_status<T: AsRef<str>>(&self, name: T) -> EmojiExistenceKind {
        match self.0.get(name.as_ref()) {
            Some(emoji) => {
                if emoji.alias_for.is_empty() {
                    EmojiExistenceKind::Exists
                } else {
                    EmojiExistenceKind::ExistsAsAliasFor(emoji.alias_for.clone())
                }
            }
            None => EmojiExistenceKind::DoesNotExist,
        }
    }

    pub async fn from_new_emoji_stream(client: Rc<SlackClient>) -> Self {
        let mut collection = Self::new();

        let stream = new_emoji_stream(client.clone(), None);
        pin_mut!(stream);

        while let Some(Ok(emoji)) = stream.next().await {
            collection.insert(emoji);
        }

        collection
    }
}

pub fn new_emoji_stream(
    slack_client: Rc<SlackClient>,
    stream_parameters: Option<EmojiStreamParameters>,
) -> impl Stream<Item = Result<Emoji, Box<dyn Error>>> {
    try_stream! {
        let parameters = stream_parameters.unwrap_or_default();
        let mut current_page_number = parameters.starting_page_number;
        let mut available_pages_count: Option<u16> = None;
        let mut pages_fetched: u16 = 0;
        loop {
            if let Some(available_pages_count) = available_pages_count {
                if current_page_number > available_pages_count {
                    break;
                }
            }
            if let Some(limit_num_pages) = parameters.limit_num_pages {
                if pages_fetched >= limit_num_pages {
                    break;
                }
            }

            let (emojis, num_pages) = slack_client.fetch_custom_emoji_page(current_page_number, parameters.num_emojis_per_page).await?;
            if available_pages_count.is_none() {
                available_pages_count = Some(num_pages);
            }
            for emoji in emojis {
                yield emoji;
            }
            current_page_number += 1;
            pages_fetched += 1;
        }
    }
}
