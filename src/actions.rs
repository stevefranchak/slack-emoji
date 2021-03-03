use std::error::Error;
use std::rc::Rc;

use colored::Colorize;
use futures::pin_mut;
use futures::stream::StreamExt;
use log::{error, trace, warn};

use crate::archive::{EmojiDirectory, EmojiFile};
use crate::emoji::{EmojiExistenceKind, EmojiPaginator};
use crate::slack::SlackClient;

// See build.rs
include!(concat!(env!("OUT_DIR"), "/emoji_standard_shortcodes.rs"));

pub async fn export<T: AsRef<str>>(
    client: Rc<SlackClient>,
    target_directory: T,
) -> Result<(), Box<dyn Error>> {
    let stream = EmojiPaginator::new(client.clone(), 100).into_stream();
    pin_mut!(stream);

    let mut emoji_directory = EmojiDirectory::new(target_directory.as_ref());
    emoji_directory.ensure_exists().await;

    while let Some(emoji_result) = stream.next().await {
        match emoji_result {
            Ok(emoji) => {
                EmojiFile::from(emoji)
                    .download_to_directory(client.clone(), &mut emoji_directory)
                    .await?
            }
            Err(e) => error!("Failed to fetch emoji list or parse response: {}", e),
        }
    }

    Ok(())
}

pub async fn import<T: AsRef<str>>(
    client: Rc<SlackClient>,
    target_directory: T,
) -> Result<(), Box<dyn Error>> {
    let mut emoji_directory = EmojiDirectory::new(target_directory.as_ref());
    match emoji_directory.exists().await {
        Ok(false) => panic!("\"{}\" is not a directory", target_directory.as_ref()),
        Err(e) => panic!(
            "Failed to check existence of directory \"{}\": {}",
            target_directory.as_ref(),
            e
        ),
        _ => (),
    };

    let existing_emoji_collection = EmojiPaginator::new(client.clone(), 100)
        .into_collection()
        .await;

    let stream = emoji_directory.stream_emoji_files();
    pin_mut!(stream);

    while let Some(Ok(emoji_file)) = stream.next().await {
        trace!("Attempting to import emoji: {:?}", emoji_file);
        if EMOJI_STANDARD_SHORTCODES.contains::<str>(&emoji_file.emoji.name) {
            warn!(
                "{}: {}",
                "Cannot import due to conflicting Slack short code name (Unicode emoji standard)"
                    .bright_red(),
                emoji_file.emoji.name.yellow()
            );
            continue;
        }
        match existing_emoji_collection.get_existence_status(&emoji_file.emoji.name) {
            EmojiExistenceKind::EmojiExists => {
                println!("Emoji {} exists on remote", emoji_file.emoji.name)
            }
            EmojiExistenceKind::EmojiExistsAsAliasFor(alias_for) => {
                println!(
                    "Emoji {} exists on remote as an alias for {}",
                    emoji_file.emoji.name, alias_for
                )
            }
            _ => (),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_standard_shortcodes() {
        assert!(EMOJI_STANDARD_SHORTCODES.contains::<str>("seal"));
        assert!(EMOJI_STANDARD_SHORTCODES.contains::<str>("female_elf"));
        assert!(!EMOJI_STANDARD_SHORTCODES.contains::<str>("bogogogogogo"));
    }
}
