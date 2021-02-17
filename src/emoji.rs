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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parsed_response.paging.count, 2);
        assert_eq!(parsed_response.paging.total, 915);
        assert_eq!(parsed_response.paging.page, 1);
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
