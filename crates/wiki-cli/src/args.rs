use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "wiki-cli")]
#[command(about = "Auto LLM Wiki CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, global = true, default_value = "wiki")]
    pub wiki_dir: PathBuf,

    #[arg(long, global = true, default_value_t = false)]
    pub sync_wiki: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Ingest(IngestArgs),
    FileClaim(FileClaimArgs),
    Supersede(SupersedeArgs),
    Query(QueryArgs),
    Lint,
    SyncIndex,
    RebuildFts,
    RebuildGraph,
    ProviderHealth,
    Outbox(OutboxCommand),
    LlmSmoke(LlmSmokeArgs),
}

#[derive(Debug, Args)]
pub struct IngestArgs {
    pub source: String,
    pub content: String,

    #[arg(long, default_value = "private:default")]
    pub scope: String,
}

#[derive(Debug, Args)]
pub struct FileClaimArgs {
    pub text: String,

    #[arg(long, default_value = "semantic")]
    pub tier: String,
}

#[derive(Debug, Args)]
pub struct SupersedeArgs {
    pub claim_id: String,
    pub text: String,

    #[arg(long, default_value_t = 0.9)]
    pub confidence: f64,

    #[arg(long = "quality-score", default_value_t = 0.9)]
    pub quality_score: f64,
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    pub query: String,

    #[arg(long, default_value_t = false)]
    pub write_page: bool,

    #[arg(long)]
    pub page_title: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum OutboxSubcommand {
    Export(OutboxExportArgs),
    Ack(OutboxAckArgs),
}

#[derive(Debug, Args)]
pub struct OutboxCommand {
    #[command(subcommand)]
    pub command: OutboxSubcommand,
}

#[derive(Debug, Args)]
pub struct OutboxExportArgs {
    #[arg(long, default_value = "wiki-cli")]
    pub consumer: String,
}

#[derive(Debug, Args)]
pub struct OutboxAckArgs {
    pub event_id: String,

    #[arg(long, default_value = "wiki-cli")]
    pub consumer: String,
}

#[derive(Debug, Args)]
pub struct LlmSmokeArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "Say 'ok' only.")]
    pub prompt: String,
}
