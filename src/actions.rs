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

    // TODO: separate consuming of stream results into a separate task (or task pool); measure perf
    while let Some(Ok(emoji)) = stream.next().await {
        EmojiFile::new(emoji)
            .download_to_directory(client.clone(), &mut emoji_directory)
            .await?;
    }

    Ok(())
}

pub async fn import() -> Result<(), Box<dyn Error>> {
    unimplemented!("Import subcommand not implemented yet!")
}
