mod arguments;

use std::{env, fs::File, io::Write, path::Path, process::exit};

use arguments::{Arguments, Verbosity};
use clap::Parser;
use imap::types::Fetch;
use indicatif::{HumanBytes, MultiProgress, ProgressBar};
use indicatif_log_bridge::LogWrapper;
use log::{debug, error};
use native_tls::TlsConnector;
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

type UnitResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type MultiProgressResult = Result<MultiProgress, Box<dyn std::error::Error + Send + Sync>>;

const IMAP_USERNAME: &str = "IMAP_USERNAME";
const IMAP_PASSWORD: &str = "IMAP_PASSWORD";

fn setup_logging(verbosity: Verbosity) -> MultiProgressResult {
    let filter = verbosity.to_filter();

    let logger = env_logger::builder()
        .filter_level(filter)
        .format_level(true)
        .format_target(false)
        .format_module_path(false)
        .format_timestamp_secs()
        .parse_default_env()
        .build();

    let multi_progress = MultiProgress::new();

    LogWrapper::new(multi_progress.clone(), logger).try_init()?;

    Ok(multi_progress)
}

fn save_messages(messages: &Vec<Fetch>, name: &str, multi_progress: &MultiProgress, writer: &mut ZipWriter<File>, options: SimpleFileOptions) -> UnitResult {
    writer.add_directory(name, options)?;

    let count = messages.len() as u64;
    let progress = multi_progress.add(ProgressBar::new(count));

    for message in messages {
        if let Some(body) = message.body() {
            let mut digest = Sha256::new();

            digest.update(body);

            let result = digest.finalize();
            let hex = hex::encode(&result);
            let filename = format!("{hex}.eml");
            let path = Path::new(name).join(filename);
            let size = HumanBytes(body.len() as u64);

            writer.start_file_from_path(&path, options)?;
            writer.write_all(body)?;

            debug!("{size} -> {path:?}");
        };

        progress.inc(1);
    }

    progress.finish();
    multi_progress.remove(&progress);

    Ok(())
}

fn main() -> UnitResult {
    let arguments = Arguments::parse();

    let multi_progress = setup_logging(arguments.verbosity)?;

    if dotenv::dotenv().ok() == None {
        debug!("Failed to load credentials from dotfile");
    }

    let tls = TlsConnector::builder().danger_accept_invalid_certs(arguments.authentication.insecure).build()?;
    let address = (arguments.hostname.as_str(), arguments.port);
    let client = imap::connect_starttls(address, &arguments.hostname, &tls)?;

    let username = arguments.authentication.username.or(env::var(IMAP_USERNAME).ok());
    let password = arguments.authentication.password.or(env::var(IMAP_PASSWORD).ok());

    if username.is_none() || password.is_none() {
        error!("Must provide a username and password");

        exit(1);
    }

    let mut session = client.login(username.unwrap(), password.unwrap()).map_err(|error| error.0)?;

    let path = Path::new(&arguments.output);
    let file = File::options().create(true).write(true).truncate(true).open(path)?;

    let mut writer = ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Zstd)
        .compression_level(Some(3))
        .unix_permissions(0o755);

    for name in &session.list(Some(""), Some("*"))? {
        let name = name.name();

        session.examine(name)?;

        match session.fetch("1:*", "RFC822") {
            Ok(messages) => save_messages(&messages, name, &multi_progress, &mut writer, options)?,
            Err(error) => error!("Failed to fetch {name}: {error}")
        }
    }

    writer.finish()?;

    Ok(())
}
