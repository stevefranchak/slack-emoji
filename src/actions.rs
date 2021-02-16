use std::error::Error;
use std::rc::Rc;

use futures::pin_mut;
use futures::stream::StreamExt;

use crate::archive::{EmojiDirectory, EmojiFile};
use crate::emoji::EmojiPaginator;
use crate::slack::SlackClient;

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
            Err(e) => eprintln!("Failed to fetch emoji list or parse response: {}", e),
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

    let stream = emoji_directory.stream_emoji_files();
    pin_mut!(stream);

    while let Some(Ok(emoji_file)) = stream.next().await {
        println!("{:?}", emoji_file);
    }

    Ok(())
}
