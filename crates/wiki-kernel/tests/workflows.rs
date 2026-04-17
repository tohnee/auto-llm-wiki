use std::fs;

use tempfile::tempdir;
use wiki_core::{Claim, ClaimId, LintIssueCode, MemoryTier};
use wiki_kernel::{QueryOptions, WikiEngine};
use wiki_storage::SqliteWikiRepository;

#[test]
fn query_flow_writes_analysis_page_when_requested() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    repo.store_claim(
        "tester",
        &Claim::new(
            ClaimId::new(),
            "Redis default TTL is 3600 seconds",
            MemoryTier::Semantic,
        ),
    )
    .expect("seed claim");

    let temp = tempdir().expect("temp dir");
    let engine = WikiEngine::new(repo, temp.path().join("wiki")).expect("engine");

    let result = engine
        .query(
            "tester",
            "Redis TTL",
            QueryOptions {
                write_page: true,
                page_title: Some("analysis-redis".to_owned()),
            },
        )
        .expect("query");

    let page = temp.path().join("wiki/pages/analysis-redis.md");
    let page_body = fs::read_to_string(page).expect("written page");

    assert_eq!(result.claims.len(), 1);
    assert!(page_body.contains("Redis default TTL is 3600 seconds"));
    assert!(page_body.contains("Redis TTL"));
}

#[test]
fn lint_flow_writes_report_for_broken_links() {
    let repo = SqliteWikiRepository::open_in_memory().expect("repo");
    let temp = tempdir().expect("temp dir");
    let wiki_dir = temp.path().join("wiki");
    let engine = WikiEngine::new(repo, wiki_dir.clone()).expect("engine");

    fs::write(
        wiki_dir.join("pages/broken.md"),
        "# Broken\n\nThis references [[missing-page]].\n",
    )
    .expect("write page");

    let issues = engine.run_lint("tester").expect("lint");
    let report = fs::read_to_string(wiki_dir.join("reports/lint-latest.md")).expect("report");

    assert!(issues.iter().any(|issue| issue.code == LintIssueCode::BrokenWikiLink));
    assert!(report.contains("missing-page"));
}
