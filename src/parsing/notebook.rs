//! Code-only notebook projection shared by indexing and every source read.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const CHUNKER_VERSION: i64 = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellSource {
    pub cell_index: usize,
    pub cell_id: Option<String>,
    pub line_start: u32,
    pub line_end: u32,
    pub code_hash: String,
}

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub content: String,
    pub source_hash: String,
    pub cells: Vec<CellSource>,
}

#[derive(Deserialize)]
struct Notebook {
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
struct Cell {
    cell_type: String,
    #[serde(default)]
    source: Option<CellText>,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CellText {
    Text(String),
    Lines(Vec<String>),
}

pub fn is_notebook(file: &str) -> bool {
    Path::new(file)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ipynb"))
}

pub fn hash(content: &[u8]) -> String {
    format!("{:x}", Sha256::digest(content))
}

pub fn chunker_version(file: &str) -> i64 {
    if is_notebook(file) {
        CHUNKER_VERSION
    } else {
        super::chunker::CHUNKER_VERSION
    }
}

pub fn from_source(file: &str, raw: &str) -> Result<SourceSnapshot> {
    if !is_notebook(file) {
        return Ok(SourceSnapshot {
            content: raw.to_owned(),
            source_hash: hash(raw.as_bytes()),
            cells: vec![],
        });
    }
    // Unknown fields (including outputs and image data) are discarded by serde.
    let notebook: Notebook = serde_json::from_str(raw).context("invalid notebook JSON/schema")?;
    let mut content = String::new();
    let mut cells = Vec::new();
    let mut next_line = 1u32;
    for (cell_index, cell) in notebook.cells.into_iter().enumerate() {
        match cell.cell_type.as_str() {
            "markdown" | "raw" => continue,
            "code" => {}
            _ => bail!("invalid notebook cell type at cell {cell_index}"),
        }
        let code = match cell.source {
            Some(CellText::Text(text)) => text,
            Some(CellText::Lines(lines)) => lines.concat(),
            None => bail!("missing notebook code source at cell {cell_index}"),
        };
        if code.is_empty() {
            continue;
        }
        let count = u32::try_from(code.lines().count()).context("notebook has too many lines")?;
        let line_end = next_line
            .checked_add(count.saturating_sub(1))
            .context("notebook line range overflow")?;
        cells.push(CellSource {
            cell_index,
            cell_id: cell.id,
            line_start: next_line,
            line_end,
            code_hash: hash(code.as_bytes()),
        });
        content.push_str(&code);
        if !code.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        next_line = line_end
            .checked_add(2)
            .context("notebook line range overflow")?;
    }
    // Cell identity/position belongs to the version; output/Markdown content does not.
    let version = serde_json::to_vec(&(&content, &cells))?;
    Ok(SourceSnapshot {
        content,
        source_hash: hash(&version),
        cells,
    })
}

pub fn read(file: &str) -> Result<SourceSnapshot> {
    let raw =
        std::fs::read_to_string(file).with_context(|| format!("cannot read source: {file}"))?;
    from_source(file, &raw)
}

impl SourceSnapshot {
    pub fn numbered(&self, start: u32, end: u32) -> Result<String> {
        let lines: Vec<_> = self.content.lines().collect();
        let lo = start.saturating_sub(1) as usize;
        let hi = (end as usize).min(lines.len());
        if start == 0 || end < start || lo >= hi {
            bail!("source line range {start}-{end} is out of bounds");
        }
        Ok(lines[lo..hi]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}: {line}", lo + i + 1))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    pub fn split_chunks(&self, chunks: Vec<super::chunker::Chunk>) -> Vec<super::chunker::Chunk> {
        let lines: Vec<_> = self.content.lines().collect();
        let mut split = Vec::new();
        for chunk in chunks {
            for cell in &self.cells {
                let start = chunk.line_start.max(cell.line_start);
                let end = chunk.line_end.min(cell.line_end);
                if start > end {
                    continue;
                }
                let content = lines[(start - 1) as usize..end as usize].join("\n");
                if content.trim().is_empty() {
                    continue;
                }
                split.push(super::chunker::Chunk {
                    file: chunk.file.clone(),
                    line_start: start,
                    line_end: end,
                    content,
                    symbol_ref: chunk.symbol_ref.clone(),
                });
            }
        }
        split
    }
}

pub fn same_cell(
    file: &str,
    left_start: u32,
    left_end: u32,
    right_start: u32,
    right_end: u32,
) -> bool {
    if !is_notebook(file) {
        return true;
    }
    let Ok(snapshot) = read(file) else {
        return false;
    };
    snapshot.cells.iter().any(|cell| {
        left_start >= cell.line_start
            && left_end <= cell.line_end
            && right_start >= cell.line_start
            && right_end <= cell.line_end
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn code_version_ignores_outputs_and_marks_cell_identity() {
        let mut raw = json!({"cells": [{"cell_type":"code","id":"a","source":["x = 1\n"],
            "outputs":[{"text":"OUTPUT_SENTINEL"}]}]});
        let a = from_source("x.ipynb", &raw.to_string()).unwrap();
        raw["cells"][0]["outputs"] = json!([{"text":"different output"}]);
        let b = from_source("x.ipynb", &raw.to_string()).unwrap();
        assert_eq!(a.source_hash, b.source_hash);
        assert_eq!(a.cells[0].code_hash, hash(b"x = 1\n"));
        raw["cells"][0]["id"] = json!("changed");
        assert_ne!(
            a.source_hash,
            from_source("x.ipynb", &raw.to_string())
                .unwrap()
                .source_hash
        );
        assert_eq!(a.numbered(1, 1).unwrap(), "1: x = 1");
    }

    #[test]
    fn schema_errors_and_empty_code_are_distinct() {
        for raw in [
            "not JSON",
            "{}",
            r#"{"cells":[{"cell_type":"code"}]}"#,
            r#"{"cells":[{"cell_type":"code","source":[7]}]}"#,
        ] {
            assert!(from_source("x.ipynb", raw).is_err());
        }
        assert!(
            from_source("x.ipynb", r#"{"cells":[]}"#)
                .unwrap()
                .content
                .is_empty()
        );
    }

    #[test]
    fn source_keeps_magics_unicode_and_maps_cells_without_json_offsets() {
        let snapshot = from_source(
            "x.ipynb",
            &json!({"cells":[
                {"cell_type":"markdown","source":"excluded"},
                {"cell_type":"code","source":["%matplotlib inline\n","!echo chào\n"]},
                {"cell_type":"raw","source":"excluded"},
                {"cell_type":"code","source":"answer = 42"}
            ]})
            .to_string(),
        )
        .unwrap();
        assert_eq!(snapshot.cells[0].cell_index, 1);
        assert_eq!(snapshot.cells[0].line_start, 1);
        assert_eq!(snapshot.cells[0].line_end, 2);
        assert_eq!(snapshot.cells[1].cell_index, 3);
        assert_eq!(
            snapshot.numbered(1, 2).unwrap(),
            "1: %matplotlib inline\n2: !echo chào"
        );
        assert_eq!(snapshot.numbered(4, 4).unwrap(), "4: answer = 42");
    }
}
