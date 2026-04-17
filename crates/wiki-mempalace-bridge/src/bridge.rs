use wiki_core::Claim;
use wiki_storage::{Result, SqliteWikiRepository, StoredPage, StoredSource};

pub struct MempalaceGraphBridge<'a> {
    repo: &'a SqliteWikiRepository,
}

impl<'a> MempalaceGraphBridge<'a> {
    pub fn new(repo: &'a SqliteWikiRepository) -> Self {
        Self { repo }
    }

    pub fn rebuild(&self) -> Result<()> {
        self.repo.clear_graph()?;
        for source in self.repo.list_sources()? {
            self.sync_source(&source)?;
        }
        for claim in self.repo.list_claims()? {
            self.sync_claim(&claim)?;
        }
        for page in self.repo.list_pages()? {
            self.sync_page(&page)?;
        }
        Ok(())
    }

    pub fn sync_claim(&self, claim: &Claim) -> Result<()> {
        let node_id = format!("claim:{}", claim.id);
        self.repo.upsert_graph_node(
            &node_id,
            "claim",
            &claim.id.to_string(),
            &claim.text,
            None,
        )?;
        self.sync_concepts(&node_id, &claim.text, "mentions")
    }

    pub fn sync_source(&self, source: &StoredSource) -> Result<()> {
        let node_id = format!("source:{}", source.id);
        self.repo
            .upsert_graph_node(&node_id, "source", &source.uri, &source.content, None)?;
        self.sync_concepts(&node_id, &source.content, "derived_from")
    }

    pub fn sync_page(&self, page: &StoredPage) -> Result<()> {
        let node_id = format!("page:{}", page.slug);
        let label = format!("{} {}", page.title, page.body);
        self.repo
            .upsert_graph_node(&node_id, "page", &page.slug, &label, None)?;
        self.sync_concepts(&node_id, &label, "linked_to")
    }

    fn sync_concepts(&self, from_node: &str, text: &str, edge_type: &str) -> Result<()> {
        for token in tokenize(text) {
            let concept_node = format!("concept:{token}");
            self.repo
                .upsert_graph_node(&concept_node, "concept", &token, &token, None)?;
            self.repo.upsert_graph_edge(
                &format!("{from_node}:{edge_type}:{concept_node}"),
                from_node,
                &concept_node,
                edge_type,
                1.0,
                None,
            )?;
        }
        Ok(())
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.len() >= 4)
        .map(ToOwned::to_owned)
        .collect()
}
