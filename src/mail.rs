use std::{io::{Read, Write}, path::Path};

use chrono::{Local, TimeDelta};
use imap::{types::Fetch, Session};
use indicatif::{HumanBytes, MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use log::{debug, info, warn};
use sha2::{Digest, Sha256};
use tar::{Builder, Header};

type IntegerResult = Result<u64, Box<dyn std::error::Error + Send + Sync>>;
type TupleResult = Result<(u64, TimeDelta), Box<dyn std::error::Error + Send + Sync>>;

const PROGRESS_STYLE_TEMPLATE: &str = "[{elapsed_precise}] {wide_bar:.cyan/blue} {pos}/{len} {msg}";

lazy_static! {
    static ref PROGRESS_STYLE: ProgressStyle = ProgressStyle::with_template(PROGRESS_STYLE_TEMPLATE)
        .unwrap()
        .progress_chars("#>-");
}

fn write_messages<W: Write>(messages: & Vec<Fetch>, name: & str, multi_progress: & MultiProgress, builder: & mut Builder<W>) -> IntegerResult {
    let count = messages.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(count));

    progress.set_style(PROGRESS_STYLE.clone());

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

pub fn fetch_messages<T: Write + Read, W: Write>(session: &mut Session<T>, multi_progress: MultiProgress, builder: &mut Builder<W>) -> TupleResult {
    let start = Local::now();
    let messages = session.list(Some(""), Some("*"))?;
    let count = messages.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(count));

    progress.set_style(PROGRESS_STYLE.clone());

    let mut total: u64 = 0;

    for name in &messages {
        let index = progress.position() + 1;
        let name = name.name();

        progress.set_message(format!("{name} [{}]", HumanBytes(total)));

        session.examine(name)?;

        match session.fetch("1:*", "RFC822") {
            Ok(messages) => {
                let size = write_messages(&messages, name, &multi_progress, builder)?;

                total += size;

                info!("{index}/{count} -> {name} [{}]", HumanBytes(size));
            },
            Err(error) => warn!("{index}/{count} -> Skipping {name}: {error}")
        }

        progress.inc(1);
    }

    let end = Local::now();
    let elapsed = end - start;

    progress.finish_and_clear();
    multi_progress.remove(&progress);

    Ok((total, elapsed))
}
