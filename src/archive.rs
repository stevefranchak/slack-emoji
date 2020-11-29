use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_compat_02::FutureExt;

use crate::emoji::Emoji;
use crate::slack::SlackClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct EmojiFile {
    #[serde(flatten)]
    emoji: Emoji,
    filename: String,
}

impl EmojiFile {
    pub fn new(emoji: Emoji) -> Self {
        let filename = Self::generate_filename_from_url(&emoji.url);
        Self {emoji, filename}
    }

    fn generate_filename_from_url<T: AsRef<str>>(url: T) -> String {
        let url = url.as_ref().to_string();
        let filename_parts: Vec<&str> = url.rsplitn(3, '/').take(2).collect();
        format!("{}-{}", filename_parts[1], filename_parts[0])
    }

    // TODO: create an EmojiArchive struct that can be passed in here
    // TODO: handle errors gracefully
    pub async fn download_to_file(&self, client: Rc<SlackClient>, parent_directory: &PathBuf) -> Result<(), Box<dyn Error>> {
        let mut emoji_file = fs::File::create(&self.get_filepath(parent_directory)).await?;
        let mut stream = client.client.get(&self.emoji.url).send().compat().await?.bytes_stream();

        while let Some(Ok(chunk)) = stream.next().await {
            emoji_file.write_all(&chunk).await?;
        }
        emoji_file.flush().await?;

        Ok(())
    }

    fn get_filepath(&self, parent_directory: &PathBuf) -> PathBuf {
        let mut filepath = parent_directory.clone();
        filepath.push(&self.filename);
        filepath
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename_from_url() {
        assert_eq!(
            EmojiFile::generate_filename_from_url("https://sub.slack.com/T03C6/zuck/6f285f21ac5f972b.png"),
            String::from("zuck-6f285f21ac5f972b.png")
        );
    }
}