use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::fs::{create_dir_all, File, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::emoji::Emoji;
use crate::slack::SlackClient;

static EMOJI_METADATA_FILENAME: &str = "metadata.ndjson";

#[derive(Debug)]
pub struct EmojiDirectory {
    path: PathBuf,
    _metadata_file_handle: Option<File>,
}

impl EmojiDirectory {
    pub fn new<T>(path: T) -> Self
    where
        T: Into<PathBuf>,
    {
        Self {
            path: path.into(),
            _metadata_file_handle: None,
        }
    }

    pub async fn ensure_exists(&self) {
        create_dir_all(&self.path)
            .await
            .unwrap_or_else(|e| panic!("Could not create EmojiDirectory {:?}: {}", &self, e))
    }

    pub fn get_inner_filepath<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }

    pub fn get_metadata_filepath(&self) -> PathBuf {
        self.get_inner_filepath(EMOJI_METADATA_FILENAME)
    }

    pub async fn open_metadata_file(&mut self) -> io::Result<&mut File> {
        if self._metadata_file_handle.is_none() {
            self._metadata_file_handle = Some(
                OpenOptions::new()
                    .append(true)
                    .read(true)
                    .create(true)
                    .open(self.get_metadata_filepath())
                    .await?,
            );
        }
        Ok(self._metadata_file_handle.as_mut().unwrap())
    }

    pub async fn record_metadata_for_emoji(&mut self, emoji_file: &EmojiFile) -> io::Result<()> {
        let metadata_file = self.open_metadata_file().await?;
        let mut emoji_bytes = serde_json::to_vec(&emoji_file)?;
        emoji_bytes.extend_from_slice(b"\n");
        metadata_file.write_all(&emoji_bytes).await?;
        metadata_file.flush().await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmojiFile {
    #[serde(flatten)]
    emoji: Emoji,
    filename: String,
}

impl EmojiFile {
    pub fn new(emoji: Emoji) -> Self {
        let filename = Self::generate_filename_from_url(&emoji.url);
        Self { emoji, filename }
    }

    fn generate_filename_from_url<T: AsRef<str>>(url: T) -> String {
        let url = url.as_ref().to_string();
        let filename_parts: Vec<&str> = url.rsplitn(3, '/').take(2).collect();
        format!("{}-{}", filename_parts[1], filename_parts[0])
    }

    // TODO: handle errors gracefully
    // TODO: refactor - function doing too much
    // TODO: refactor - need to handle metadata file diffs intelligently
    pub async fn download_to_directory(
        &self,
        client: Rc<SlackClient>,
        directory: &mut EmojiDirectory,
    ) -> Result<(), Box<dyn Error>> {
        let emoji_filepath = directory.get_inner_filepath(&self.filename);
        if !emoji_filepath.is_file() {
            let mut emoji_file = File::create(&emoji_filepath).await?;
            let mut stream = client
                .client
                .get(&self.emoji.url)
                .send()
                .await?
                .bytes_stream();

            while let Some(Ok(chunk)) = stream.next().await {
                emoji_file.write_all(&chunk).await?;
            }
            emoji_file.flush().await?;

            // TODO - putting this here is a kludge for now - should prob handle in caller and use enum DownloadState
            directory.record_metadata_for_emoji(&self).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename_from_url() {
        assert_eq!(
            EmojiFile::generate_filename_from_url(
                "https://sub.slack.com/T03C6/zuck/6f285f21ac5f972b.png"
            ),
            String::from("zuck-6f285f21ac5f972b.png")
        );
    }
}
