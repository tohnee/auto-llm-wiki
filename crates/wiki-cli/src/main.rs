mod args;

use clap::Parser;
use thiserror::Error;
use uuid::Uuid;
use wiki_core::{ClaimId, MemoryTier};
use wiki_kernel::{QueryOptions, WikiEngine};
use wiki_storage::SqliteWikiRepository;

use crate::args::{
    Cli, Command, FileClaimArgs, IngestArgs, LlmSmokeArgs, OutboxAckArgs, OutboxCommand,
    OutboxExportArgs, OutboxSubcommand, QueryArgs, SupersedeArgs,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    let db = cli.db.ok_or(CliError::MissingDbPath)?;
    let repo = SqliteWikiRepository::open(&db)?;
    let engine = WikiEngine::new(repo, &cli.wiki_dir)?;

    match cli.command {
        Command::Ingest(args) => ingest(&engine, args)?,
        Command::FileClaim(args) => file_claim(&engine, args)?,
        Command::Supersede(args) => supersede(&engine, args)?,
        Command::Query(args) => query(&engine, args)?,
        Command::Lint => lint(&engine)?,
        Command::Outbox(args) => outbox(&engine, args)?,
        Command::LlmSmoke(args) => llm_smoke(args),
    }

    Ok(())
}

fn ingest(engine: &WikiEngine, args: IngestArgs) -> Result<(), CliError> {
    let claim = engine.ingest("cli", &args.source, &args.content, &args.scope)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "claim_id": claim.id.to_string(),
            "tier": "episodic"
        }))?
    );
    Ok(())
}

fn file_claim(engine: &WikiEngine, args: FileClaimArgs) -> Result<(), CliError> {
    let tier = parse_tier(&args.tier);
    let claim = engine.file_claim("cli", &args.text, tier)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "claim_id": claim.id.to_string(),
            "tier": args.tier
        }))?
    );
    Ok(())
}

fn supersede(engine: &WikiEngine, args: SupersedeArgs) -> Result<(), CliError> {
    let replacement = engine.supersede(
        "cli",
        ClaimId::parse(&args.claim_id)?,
        &args.text,
        args.confidence,
        args.quality_score,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "previous": replacement.previous.id.to_string(),
            "current": replacement.current.id.to_string()
        }))?
    );
    Ok(())
}

fn query(engine: &WikiEngine, args: QueryArgs) -> Result<(), CliError> {
    let result = engine.query(
        "cli",
        &args.query,
        QueryOptions {
            write_page: args.write_page,
            page_title: args.page_title,
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "claims": result.claims.iter().map(|claim| serde_json::json!({
                "id": claim.id.to_string(),
                "text": claim.text,
            })).collect::<Vec<_>>(),
            "page_path": result.page_path.map(|path| path.to_string_lossy().to_string()),
        }))?
    );
    Ok(())
}

fn lint(engine: &WikiEngine) -> Result<(), CliError> {
    let issues = engine.run_lint("cli")?;
    println!("{}", serde_json::to_string_pretty(&issues)?);
    Ok(())
}

fn outbox(engine: &WikiEngine, args: OutboxCommand) -> Result<(), CliError> {
    match args.command {
        OutboxSubcommand::Export(OutboxExportArgs { consumer }) => {
            let events = engine.export_outbox(&consumer)?;
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        OutboxSubcommand::Ack(OutboxAckArgs { event_id, consumer }) => {
            engine.ack_outbox(&consumer, Uuid::parse_str(&event_id)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "consumer": consumer,
                    "event_id": event_id,
                    "acked": true
                }))?
            );
        }
    }
    Ok(())
}

fn llm_smoke(_args: LlmSmokeArgs) {
    println!("ok");
}

fn parse_tier(raw: &str) -> MemoryTier {
    match raw.to_ascii_lowercase().as_str() {
        "working" => MemoryTier::Working,
        "episodic" => MemoryTier::Episodic,
        "procedural" => MemoryTier::Procedural,
        _ => MemoryTier::Semantic,
    }
}

#[derive(Debug, Error)]
enum CliError {
    #[error("cli io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("storage error: {0}")]
    Storage(#[from] wiki_storage::StorageError),
    #[error("kernel error: {0}")]
    Kernel(#[from] wiki_kernel::KernelError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("uuid error: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("missing required --db path")]
    MissingDbPath,
}
