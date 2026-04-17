use std::{collections::HashSet, fs, path::Path};

use wiki_core::{Claim, LintIssue, LintIssueCode, LintSeverity};

pub fn run_lint(wiki_dir: &Path, claims: &[Claim]) -> std::io::Result<Vec<LintIssue>> {
    let pages_dir = wiki_dir.join("pages");
    let mut page_slugs = HashSet::new();
    let mut page_bodies = Vec::new();
    let mut referenced = HashSet::new();
    let mut issues = Vec::new();

    if pages_dir.exists() {
        for entry in fs::read_dir(&pages_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let slug = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned();
            let body = fs::read_to_string(&path)?;
            for link in extract_links(&body) {
                referenced.insert(link.clone());
                if !pages_dir.join(format!("{link}.md")).exists() {
                    issues.push(LintIssue {
                        code: LintIssueCode::BrokenWikiLink,
                        severity: LintSeverity::Error,
                        page_title: Some(slug.clone()),
                        claim_id: None,
                        message: format!("page contains missing wikilink [[{link}]]"),
                    });
                }
            }
            page_slugs.insert(slug);
            page_bodies.push(body);
        }
    }

    for slug in &page_slugs {
        if !referenced.contains(slug) {
            issues.push(LintIssue {
                code: LintIssueCode::OrphanPage,
                severity: LintSeverity::Warning,
                page_title: Some(slug.clone()),
                claim_id: None,
                message: format!("page `{slug}` has no incoming wikilinks"),
            });
        }
    }

    let combined_pages = page_bodies.join("\n").to_lowercase();
    for claim in claims {
        if claim.stale && !combined_pages.contains(&claim.id.to_string()) {
            issues.push(LintIssue {
                code: LintIssueCode::StaleClaim,
                severity: LintSeverity::Warning,
                page_title: None,
                claim_id: Some(claim.id),
                message: format!("stale claim {} is not referenced in any page", claim.id),
            });
        }

        if let Some(keyword) = claim
            .text
            .split(|ch: char| !ch.is_alphanumeric())
            .find(|token| token.len() >= 5)
            .map(|token| token.to_lowercase())
            && !combined_pages.contains(&keyword)
        {
            issues.push(LintIssue {
                code: LintIssueCode::MissingCrossReference,
                severity: LintSeverity::Info,
                page_title: None,
                claim_id: Some(claim.id),
                message: format!("claim {} has no page cross-reference for keyword `{keyword}`", claim.id),
            });
        }
    }

    Ok(issues)
}

pub fn render_report(issues: &[LintIssue]) -> String {
    let mut report = String::from("# Lint Report\n\n");
    if issues.is_empty() {
        report.push_str("No issues detected.\n");
        return report;
    }

    for issue in issues {
        report.push_str(&format!(
            "- {:?}: {}\n",
            issue.code, issue.message
        ));
    }
    report
}

fn extract_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            links.push(after[..end].trim().to_owned());
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    links
}
