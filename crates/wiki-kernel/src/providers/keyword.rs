use wiki_core::RankedClaim;
use wiki_storage::{SqliteWikiRepository, StorageError};

pub type Result<T> = std::result::Result<T, StorageError>;

pub trait KeywordRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RankedClaim>>;
}

pub struct SqliteFtsRetriever<'a> {
    repo: &'a SqliteWikiRepository,
}

impl<'a> SqliteFtsRetriever<'a> {
    pub fn new(repo: &'a SqliteWikiRepository) -> Self {
        Self { repo }
    }
}

impl KeywordRetriever for SqliteFtsRetriever<'_> {
    fn retrieve(&self, query: &str) -> Result<Vec<RankedClaim>> {
        let claim_ids = self.repo.search_fts_claim_ids(query)?;
        Ok(claim_ids
            .into_iter()
            .enumerate()
            .map(|(index, claim_id)| RankedClaim::new(claim_id, index + 1))
            .collect())
    }
}
