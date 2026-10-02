//! Source versions and serialized budgets shared by the existing consumers.
use crate::parsing::notebook::{self, CellSource};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use surrealdb::{Surreal, engine::local::Db};

// The existing MCP output cap is a local transport budget, not a provider token limit.
pub const MAX_PAYLOAD_BYTES: usize = 48_000;

#[derive(Debug, Clone, Serialize)]
pub struct ContextBudget {
    pub schema_version: u32,
    pub max_bytes: usize,
    pub serialized_bytes: Option<usize>,
    pub scope: String,
    pub token_limit_verified: bool,
    pub truncated: bool,
    pub diagnostics_omitted: bool,
    pub omitted_evidence: Vec<String>,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            schema_version: 1,
            max_bytes: MAX_PAYLOAD_BYTES,
            serialized_bytes: None,
            scope: "final_evidence".into(),
            token_limit_verified: false,
            truncated: false,
            diagnostics_omitted: false,
            omitted_evidence: vec![],
        }
    }
}

#[derive(Serialize)]
struct FinalEvidence<'a> {
    results: &'a [super::engine::CodeResult],
    warnings: &'a [String],
    warming: bool,
    graph_pending: bool,
    budget: &'a ContextBudget,
}

fn measure_context(result: &mut super::engine::QueryResult) -> usize {
    for _ in 0..4 {
        let projection = FinalEvidence {
            results: &result.results,
            warnings: &result.warnings,
            warming: result.warming,
            graph_pending: result.graph_pending,
            budget: &result.budget,
        };
        let count = serde_json::to_vec(&projection).map_or(usize::MAX, |bytes| bytes.len());
        if result.budget.serialized_bytes == Some(count) {
            return count;
        }
        result.budget.serialized_bytes = Some(count);
    }
    result.budget.serialized_bytes.unwrap_or(usize::MAX)
}

pub fn bound_query(result: &mut super::engine::QueryResult) {
    // Console diagnostics stay intact outside the final evidence projection.
    result.budget.diagnostics_omitted = true;
    let original_warnings = result.warnings.clone();
    loop {
        result.warnings.clone_from(&original_warnings);
        if !result.budget.omitted_evidence.is_empty() {
            result.warnings.push(format!(
                "{} evidence ranges omitted; narrow the query or read the source",
                result.budget.omitted_evidence.len()
            ));
        }
        if measure_context(result) <= MAX_PAYLOAD_BYTES {
            break;
        }
        let Some(omitted) = result.results.pop() else {
            result.warnings =
                vec!["requires_split: mandatory context metadata exceeds the budget".into()];
            result.budget.omitted_evidence.clear();
            result.budget.truncated = true;
            measure_context(result);
            break;
        };
        let id = omitted
            .source
            .as_ref()
            .map(|source| source.evidence_id.clone())
            .unwrap_or_else(|| {
                notebook::hash(
                    format!(
                        "{}:{}:{}",
                        omitted.file, omitted.line_start, omitted.line_end
                    )
                    .as_bytes(),
                )
            });
        result.budget.omitted_evidence.push(id);
        result.budget.truncated = true;
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexSource {
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub notebook_cells: Option<Vec<CellSource>>,
    #[serde(default)]
    pub chunker_version: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CellRange {
    pub cell_index: usize,
    pub cell_id: Option<String>,
    pub line_start: u32,
    pub line_end: u32,
    pub code_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceSource {
    pub evidence_id: String,
    pub origin: String,
    pub freshness: String,
    pub source_hash: Option<String>,
    pub indexed_source_hash: Option<String>,
    pub line_basis: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<CellRange>,
}

pub async fn indexed_source(db: &Surreal<Db>, file: &str) -> Result<IndexSource> {
    let mut response = db.query(
        "SELECT source_hash, notebook_cells, chunker_version FROM file_meta WHERE path = $file LIMIT 1"
    ).bind(("file", file.to_owned())).await?;
    let rows: Vec<IndexSource> = response.take(0)?;
    Ok(rows.into_iter().next().unwrap_or_default())
}

pub async fn source_problem(db_map: &HashMap<String, Surreal<Db>>, file: &str) -> Option<String> {
    let Some(repo) = db_map
        .keys()
        .filter(|repo| crate::path_in_repo(file, repo))
        .max_by_key(|repo| repo.len())
    else {
        return Some("source is outside the requested repositories".into());
    };
    if let Err(error) = source_in_repo(file, repo) {
        return Some(error.to_string());
    }
    let indexed = match indexed_source(&db_map[repo], file).await {
        Ok(source) => source,
        Err(error) => return Some(format!("source version lookup failed: {error}")),
    };
    if notebook::is_notebook(file) {
        return validate_notebook(file, &indexed)
            .err()
            .map(|error| error.to_string());
    }
    match notebook::read(file) {
        Ok(snapshot)
            if indexed
                .source_hash
                .as_ref()
                .is_some_and(|hash| hash != &snapshot.source_hash) =>
        {
            Some("source version changed; reindex before retrieval".into())
        }
        Ok(_) => None,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            None
        }
        Err(error) => Some(error.to_string()),
    }
}

pub async fn filter_sources(
    chunks: Vec<super::merger::MergeChunk>,
    db_map: &HashMap<String, Surreal<Db>>,
) -> (Vec<super::merger::MergeChunk>, Vec<String>) {
    let mut versions = HashMap::new();
    let mut kept = Vec::new();
    let mut warnings = Vec::new();
    for chunk in chunks {
        if !versions.contains_key(&chunk.file) {
            versions.insert(
                chunk.file.clone(),
                source_problem(db_map, &chunk.file).await,
            );
        }
        if let Some(reason) = &versions[&chunk.file] {
            warnings.push(format!("{}: {reason}", chunk.file));
        } else {
            kept.push(chunk);
        }
    }
    warnings.sort();
    warnings.dedup();
    (kept, warnings)
}

pub fn validate_notebook(file: &str, indexed: &IndexSource) -> Result<()> {
    if !notebook::is_notebook(file) {
        return Ok(());
    }
    if indexed.chunker_version != Some(notebook::CHUNKER_VERSION) || indexed.source_hash.is_none() {
        bail!("notebook code-only index is unavailable; reindex the notebook before retrieval");
    }
    match notebook::read(file) {
        Ok(snapshot) if indexed.source_hash.as_ref() == Some(&snapshot.source_hash) => Ok(()),
        Ok(_) => bail!("notebook source version changed; reindex before retrieval"),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub fn source_in_repo(file: &str, repo: &str) -> Result<()> {
    if !crate::path_in_repo(file, repo)
        || Path::new(file)
            .components()
            .any(|p| matches!(p, std::path::Component::ParentDir))
    {
        bail!("source path escapes the repository");
    }
    if std::fs::symlink_metadata(file).is_ok() {
        let root = Path::new(repo)
            .canonicalize()
            .context("cannot resolve source repository")?;
        let actual = Path::new(file)
            .canonicalize()
            .context("cannot resolve source file")?;
        if !actual.starts_with(root) {
            bail!("source symlink escapes the repository");
        }
    }
    Ok(())
}

pub fn evidence(file: &str, start: u32, end: u32, indexed: &IndexSource) -> EvidenceSource {
    evidence_snapshot(file, start, end, indexed, notebook::read(file))
}

fn evidence_snapshot(
    file: &str,
    start: u32,
    end: u32,
    indexed: &IndexSource,
    snapshot: Result<notebook::SourceSnapshot>,
) -> EvidenceSource {
    let (origin, freshness, source_hash, cells) = match snapshot {
        Ok(snapshot) => {
            let freshness = match &indexed.source_hash {
                Some(hash) if hash == &snapshot.source_hash => "verified",
                Some(_) => "changed",
                None => "unverified",
            };
            (
                "disk",
                freshness,
                Some(snapshot.source_hash),
                snapshot.cells,
            )
        }
        Err(error) => {
            let state = if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                "missing"
            } else {
                "unavailable"
            };
            (
                "index_only",
                state,
                indexed.source_hash.clone(),
                indexed.notebook_cells.clone().unwrap_or_default(),
            )
        }
    };
    let cells = cells
        .into_iter()
        .filter_map(|cell| {
            let lo = start.max(cell.line_start);
            let hi = end.min(cell.line_end);
            (lo <= hi).then(|| CellRange {
                cell_index: cell.cell_index,
                cell_id: cell.cell_id,
                line_start: lo - cell.line_start + 1,
                line_end: hi - cell.line_start + 1,
                code_hash: cell.code_hash,
            })
        })
        .collect();
    let identity = format!(
        "{file}:{start}:{end}:{}",
        source_hash.as_deref().unwrap_or("unverified")
    );
    EvidenceSource {
        evidence_id: notebook::hash(identity.as_bytes()),
        origin: origin.into(),
        freshness: freshness.into(),
        source_hash,
        indexed_source_hash: indexed.source_hash.clone(),
        line_basis: if notebook::is_notebook(file) {
            "notebook_code_projection"
        } else {
            "file"
        }
        .into(),
        cells,
    }
}

impl EvidenceSource {
    pub fn tag(&self) -> String {
        let cells = self
            .cells
            .iter()
            .map(|cell| {
                format!(
                    "cell[{}]#L{}-{}",
                    cell.cell_id
                        .as_deref()
                        .map(|id| format!("{id:?}"))
                        .unwrap_or_else(|| cell.cell_index.to_string()),
                    cell.line_start,
                    cell.line_end
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            " [source:{} freshness:{} hash:{}{}]",
            self.origin,
            self.freshness,
            self.source_hash.as_deref().unwrap_or("unverified"),
            if cells.is_empty() {
                String::new()
            } else {
                format!(" {cells}")
            }
        )
    }
}

pub fn location(file: &str, start: u32, end: u32, source: Option<&EvidenceSource>) -> String {
    if notebook::is_notebook(file) {
        if let Some(source) = source {
            let cells = source
                .cells
                .iter()
                .map(|cell| {
                    format!(
                        "cell[{}]#L{}-{}",
                        cell.cell_id
                            .as_deref()
                            .map(|id| format!("{id:?}"))
                            .unwrap_or_else(|| cell.cell_index.to_string()),
                        cell.line_start,
                        cell.line_end
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            return format!(
                "{file}#{}{}",
                if cells.is_empty() {
                    format!("code-projection:L{start}-{end}")
                } else {
                    cells
                },
                source.tag()
            );
        }
        return format!("{file}#code-projection:L{start}-{end}");
    }
    format!(
        "{file}#L{start}-{end}{}",
        source.map(|source| source.tag()).unwrap_or_default()
    )
}

pub async fn attach_sources(
    results: &mut Vec<super::engine::CodeResult>,
    db_map: &HashMap<String, Surreal<Db>>,
) -> bool {
    let mut versions = HashMap::new();
    let before = results.len();
    for result in results.iter_mut() {
        if !versions.contains_key(&result.file) {
            let indexed = match super::find_db_for_file(db_map, &result.file) {
                Some(db) => indexed_source(db, &result.file).await.unwrap_or_default(),
                None => IndexSource::default(),
            };
            versions.insert(result.file.clone(), indexed);
        }
        let snapshot = notebook::read(&result.file);
        if let Ok(current) = &snapshot {
            if current
                .numbered(result.line_start, result.line_end)
                .ok()
                .as_deref()
                != Some(&result.content)
            {
                result.content.clear();
                continue;
            }
        }
        result.source = Some(evidence_snapshot(
            &result.file,
            result.line_start,
            result.line_end,
            &versions[&result.file],
            snapshot,
        ));
    }
    results.retain(|result| !result.content.is_empty());
    results.len() != before
}

pub fn guard_request<T: Serialize>(body: &T) -> Result<()> {
    serialized_request(body).map(|_| ())
}

pub fn serialized_request<T: Serialize>(body: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(body)?;
    let size = bytes.len();
    if size > MAX_PAYLOAD_BYTES {
        bail!(
            "requires_split: serialized request is {size} bytes, local budget is {MAX_PAYLOAD_BYTES} bytes; narrow the request"
        );
    }
    Ok(bytes)
}

pub fn tool_result(text: impl Into<String>) -> rmcp::model::CallToolResult {
    let bounded = fit_mcp_text(&text.into(), "");
    let error = bounded.starts_with("Error:") || bounded.starts_with("requires_split:");
    let mut result =
        rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(bounded)]);
    result.is_error = Some(error);
    result
}

pub fn mcp_text_bytes(text: &str) -> usize {
    let result = rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(text)]);
    serde_json::to_vec(&result).map_or(usize::MAX, |bytes| bytes.len())
}

pub fn fit_mcp_text(text: &str, suffix: &str) -> String {
    let whole = format!("{text}{suffix}");
    if mcp_text_bytes(&whole) <= MAX_PAYLOAD_BYTES {
        return whole;
    }
    const OMITTED: &str =
        "\n\n[context truncated to serialized output budget; read the source ranges above]\n";
    if mcp_text_bytes(&format!("{OMITTED}{suffix}")) > MAX_PAYLOAD_BYTES {
        return "requires_split: mandatory context exceeds the serialized output budget; narrow the request".into();
    }
    let (mut lo, mut hi) = (0, text.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let mut boundary = mid;
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let candidate = format!("{}{OMITTED}{suffix}", &text[..boundary]);
        if mcp_text_bytes(&candidate) <= MAX_PAYLOAD_BYTES {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    while !text.is_char_boundary(lo) {
        lo -= 1;
    }
    // Prefer a whole line so escaped JSON/code tokens are not cut midway.
    let cut = text[..lo].rfind('\n').unwrap_or(lo);
    format!("{}{OMITTED}{suffix}", &text[..cut])
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
