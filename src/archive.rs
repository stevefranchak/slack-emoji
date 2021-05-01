use std::collections::HashSet;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_stream::try_stream;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio::fs::{create_dir_all, metadata, File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::emoji::Emoji;
use crate::slack::SlackClient;

static EMOJI_METADATA_FILENAME: &str = "metadata.ndjson";

pub struct EmojiMetadataFile {
    handle: File,
}

impl EmojiMetadataFile {
    pub async fn open<P: AsRef<Path>>(path: P) -> io::Result<EmojiMetadataFile> {
        Ok(EmojiMetadataFile {
            handle: OpenOptions::new()
                .append(true)
                .read(true)
                .create(true)
                .open(path)
                .await?,
        })
    }

    pub async fn record_emoji(&mut self, emoji_file: &EmojiFile) -> io::Result<()> {
        let mut emoji_bytes = serde_json::to_vec(&emoji_file)?;
        emoji_bytes.extend_from_slice(b"\n");
        self.handle.write_all(&emoji_bytes).await?;
        self.handle.flush().await?;
        Ok(())
    }

    pub async fn get_emoji_name_set(&self) -> Result<HashSet<String>, Box<dyn Error>> {
        // TODO: not sure how to do this without cloning since BufReader moves `handle`
        let handle = self.handle.try_clone().await?;
        let reader = BufReader::new(handle);
        let mut lines = reader.lines();
        let mut set = HashSet::new();

        while let Some(line) = lines.next_line().await? {
            let emoji_file: EmojiFile = serde_json::from_str(&line)?;
            set.insert(emoji_file.emoji.name);
        }

        Ok(set)
    }
}

#[derive(Debug)]
pub struct EmojiDirectory {
    path: PathBuf,
}

impl EmojiDirectory {
    pub fn new<T>(path: T) -> Self
    where
        T: Into<PathBuf>,
    {
        Self { path: path.into() }
    }

    pub async fn ensure_exists(&self) {
        create_dir_all(&self.path)
            .await
            .unwrap_or_else(|e| panic!("Could not create EmojiDirectory {:?}: {}", &self, e))
    }

    pub async fn exists(&self) -> Result<bool, Box<dyn Error>> {
        Ok(metadata(&self.path).await?.is_dir())
    }

    pub fn get_inner_filepath<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }

    pub fn get_metadata_filepath(&self) -> PathBuf {
        self.get_inner_filepath(EMOJI_METADATA_FILENAME)
    }

    pub fn get_emoji_filepath(&self, emoji_file: &EmojiFile) -> PathBuf {
        self.get_inner_filepath(&emoji_file.filename)
    }

    pub async fn open_metadata_file(&self) -> io::Result<EmojiMetadataFile> {
        EmojiMetadataFile::open(self.get_metadata_filepath()).await
    }

    pub fn stream_emoji_files(&self) -> impl Stream<Item = Result<EmojiFile, Box<dyn Error + '_>>> {
        try_stream! {
            let reader = BufReader::new(self.open_metadata_file().await?.handle);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await? {
                let emoji_file: EmojiFile = serde_json::from_str(&line)?;
                yield emoji_file;
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmojiFile {
    #[serde(flatten)]
    pub emoji: Emoji,
    pub filename: String,
}

impl EmojiFile {
    fn generate_filename_from_url<S: Into<String>>(url: S) -> String {
        let url = url.into();
        let filename_parts: Vec<&str> = url.rsplitn(3, '/').take(2).collect();
        format!("{}-{}", filename_parts[1], filename_parts[0])
    }

    pub async fn download_to_directory(
        &self,
        client: Rc<SlackClient>,
        directory: &EmojiDirectory,
    ) -> Result<(), Box<dyn Error>> {
        let emoji_filepath = directory.get_inner_filepath(&self.filename);
        client.download(&self.emoji.url, &emoji_filepath).await?;
        Ok(())
    }

    pub async fn upload_from_directory(
        &self,
        client: Rc<SlackClient>,
        directory: &EmojiDirectory,
    ) -> Result<(), Box<dyn Error>> {
        client
            .upload(&self, directory.get_emoji_filepath(&self))
            .await
    }
}

impl From<Emoji> for EmojiFile {
    fn from(emoji: Emoji) -> Self {
        Self {
            filename: Self::generate_filename_from_url(&emoji.url),
            emoji,
        }
    }
}

// TODO: TEST - create temp emoji metadata file and test streaming EmojiFiles from it
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
