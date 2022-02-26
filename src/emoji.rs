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

        let stream = new_emoji_stream(client.clone());
        pin_mut!(stream);

        while let Some(Ok(emoji)) = stream.next().await {
            collection.insert(emoji);
        }

        collection
    }
}

pub fn new_emoji_stream(
    slack_client: Rc<SlackClient>,
) -> impl Stream<Item = Result<Emoji, Box<dyn Error>>> {
    try_stream! {
        let mut curr_page: u16 = 1;
        let mut known_num_pages: Option<u16> = None;
        loop {
            if let Some(num_pages) = known_num_pages {
                if curr_page > num_pages {
                    break;
                }
            }
            let (emojis, num_pages) = slack_client.fetch_custom_emoji_page(curr_page).await?;
            if known_num_pages.is_none() {
                known_num_pages = Some(num_pages);
            }
            for emoji in emojis {
                yield emoji;
            }
            curr_page += 1;
        }
    }
}
