use std::{fs::File, path::Path};

use imap::types::Fetch;
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, warn};
use sha2::{Digest, Sha256};
use tar::{Builder, Header};
use zstd::stream::AutoFinishEncoder;

type IntegerResult = Result<u64, Box<dyn std::error::Error + Send + Sync>>;

pub struct WriteTask<'a> {
    messages: &'a Vec<Fetch>,
    name: &'a str,
    multi_progress: &'a MultiProgress,
    style: &'a ProgressStyle,
    builder: &'a mut Builder<AutoFinishEncoder<'static, File>>
}

impl<'a> WriteTask<'a> {
    pub fn new(
        messages: &'a Vec<Fetch>,
        name: &'a str,
        multi_progress: &'a MultiProgress,
        style: &'a  ProgressStyle,
        builder: &'a mut Builder<AutoFinishEncoder<'static, File>>
    ) -> WriteTask<'a> {
        Self {
            messages,
            name,
            multi_progress,
            style,
            builder
        }
    }
}

pub fn write_messages(task: WriteTask) -> IntegerResult {
    let WriteTask { messages, name, multi_progress, style, builder } = task;

    let count = messages.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(count));

    progress.set_style(style.clone());

    let mut total: u64 = 0;

    for message in messages {
        let index = progress.position() + 1;

        if let Some(body) = message.body() {
            let mut digest = Sha256::new();

            digest.update(body);

            let result = digest.finalize();
            let hex = hex::encode(&result);
            let filename = format!("{hex}.eml");
            let path = &Path::new(name).join(filename);
            let size = body.len() as u64;

            let mut header = Header::new_gnu();

            header.set_size(size);
            header.set_cksum();
            header.set_mode(0o755);

            builder.append_data(&mut header, path, body)?;

            total += size;

            debug!("{index}/{count} -> {:?} [{}]", path, HumanBytes(size));

            // Show the current mailbox name and total amount of data fetched.
            progress.set_message(format!("{name} [{}]", HumanBytes(total)));
        }
        else {
            warn!("{index}/{count} -> Skipping: Unable to retrieve message body");
        }

        progress.inc(1);
    }

    progress.finish();
    multi_progress.remove(&progress);

    Ok(total)
}
