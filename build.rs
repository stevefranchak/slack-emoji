#[macro_use]
extern crate lazy_static;

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use phf_codegen::Set;
use serde::Deserialize;
use version_compare::Version;

#[derive(Deserialize)]
struct EmojiInfo {
    added_in: String,
    short_names: Vec<String>,
}

lazy_static! {
    // The current max emoji standard version supported by Slack (not sure if there's a delay before Slack
    // adopts a newer version anymore; doing this for now just to play it safe)
    static ref MAX_SUPPORTED_EMOJI_VERSION: Version<'static> = Version::from("13.0").unwrap();
}

// Slack's /api/emoji.getInfo endpoint returns the "emoji_not_found" error for short codes belonging to standard
// emojis. Only when attempting to add an emoji whose name conflicts with a standard emoji's short code does the
// Slack API return an "error_name_taken_i18n" error. It seems that Slack's customize/emoji UI first checks an
// in-memory list of standard emoji short codes before making a request to its /api/emoji.getInfo endpoint. In the
// interest of not having to fetch and compile a list of standard emoji short codes every time the slack_emoji tool
// runs, this build script will generate a Rust source file containing a single phf::Set of the standard emoji
// short codes at build time.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Would rather not include a 4 GB git submodule just for one JSON file, so we're doing this.
    // This repo is mentioned at https://emojipedia.org/slack/.
    let emojis =
        minreq::get("https://raw.githubusercontent.com/iamcal/emoji-data/master/emoji.json")
            .with_timeout(10)
            .send()?
            .json::<Vec<EmojiInfo>>()?;

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("emoji_standard_shortcodes.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let mut short_code_set: Set<&str> = Set::new();
    for short_code in emojis
        .iter()
        .filter_map(|emoji| {
            if Version::from(&emoji.added_in).unwrap() <= *MAX_SUPPORTED_EMOJI_VERSION {
                Some(&emoji.short_names)
            } else {
                None
            }
        })
        .flatten()
    {
        short_code_set.entry(short_code);
    }

    writeln!(
        &mut file,
        "static EMOJI_STANDARD_SHORTCODES: phf::Set<&'static str> = \n{};\n",
        short_code_set.build()
    )
    .unwrap();

    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
