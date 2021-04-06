use std::error::Error;
use std::rc::Rc;

use colored::Colorize;
use futures::pin_mut;
use futures::stream::StreamExt;
use log::{error, info, trace, warn};

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

    let emoji_directory = EmojiDirectory::new(target_directory.as_ref());
    emoji_directory.ensure_exists().await;
    let mut metadata_file = emoji_directory.open_metadata_file().await?;
    let metadata_emoji_name_set = metadata_file.get_emoji_name_set().await?;

    while let Some(emoji_result) = stream.next().await {
        match emoji_result {
            Ok(emoji) => {
                let emoji_file = EmojiFile::from(emoji);
                if !metadata_emoji_name_set.contains(&emoji_file.emoji.name) {
                    emoji_file
                        .download_to_directory(client.clone(), &emoji_directory)
                        .await?;
                    metadata_file.record_emoji(&emoji_file).await?;
                    info!("Downloaded emoji: {:?}", emoji_file);
                } else {
                    trace!("Emoji is already downloaded, skipping: {:?}", emoji_file);
                }
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
    let emoji_directory = EmojiDirectory::new(target_directory.as_ref());
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

    let mut aliases_to_process: Vec<EmojiFile> = vec![];

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
                info!("Emoji {} exists on remote; skipping", emoji_file.emoji.name);
                continue;
            }
            EmojiExistenceKind::EmojiExistsAsAliasFor(alias_for) => {
                info!(
                    "Emoji {} exists on remote as an alias for {}; skipping",
                    emoji_file.emoji.name, alias_for
                );
                continue;
            }
            _ => (),
        }

        // Handle aliases later to give a chance for the aliased emoji to be uploaded
        if !emoji_file.emoji.alias_for.is_empty() {
            aliases_to_process.push(emoji_file);
            continue;
        }

        if let Err(e) = emoji_file
            .upload_from_directory(client.clone(), &emoji_directory)
            .await
        {
            error!("{}; skipping", e);
        }
    }

    for alias_file in aliases_to_process {
        if let Err(e) = client
            .add_alias(&alias_file.emoji.name, &alias_file.emoji.alias_for)
            .await
        {
            error!("{}; skipping", e);
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
