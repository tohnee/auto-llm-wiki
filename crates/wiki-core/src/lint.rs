use serde::{Deserialize, Serialize};

use crate::claim::ClaimId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintIssueCode {
    BrokenWikiLink,
    OrphanPage,
    StaleClaim,
    MissingCrossReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintIssue {
    pub code: LintIssueCode,
    pub severity: LintSeverity,
    pub page_title: Option<String>,
    pub claim_id: Option<ClaimId>,
    pub message: String,
}
