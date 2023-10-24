mod db;

use clap::Parser;
use color_eyre::eyre::Result;
use std::fmt::Debug;
use std::path::PathBuf;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
enum Commands {
    Parse(ParseCommand),
}

#[derive(Debug, Parser)]
struct ParseCommand {
    #[arg(short, long)]
    path: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    color_eyre::install()?;

    let cmd = Commands::parse();
    match cmd {
        Commands::Parse(ParseCommand { path }) => {
            // polar::fun_name(path)?;
            let mut spotify_analytics = db::SpotifyAnalytics::new()?;
            let top_artists = spotify_analytics.get_top_10_artists();
            dbg!(top_artists);
            spotify_analytics
                .deserialize_extended_streaming_history_json_files_from_folder(path)?;
            spotify_analytics.save()?;
        }
    }

    Ok(())
}
