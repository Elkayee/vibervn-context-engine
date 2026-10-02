use super::*;
use serde_json::json;

#[test]
fn serialized_budget_counts_escaping_unicode_and_suffix() {
    for text in ["x\n".repeat(23_900), "界\\\"".repeat(9_000)] {
        assert!(text.len() <= MAX_PAYLOAD_BYTES);
        let bounded = fit_mcp_text(&text, "\nmandatory hint");
        assert!(mcp_text_bytes(&bounded) <= MAX_PAYLOAD_BYTES);
        assert!(bounded.ends_with("mandatory hint"));
        assert!(bounded.contains("truncated"));
    }
    assert!(guard_request(&json!({"content":"x\n".repeat(24_000)})).is_err());
    assert!(fit_mcp_text("code", &"x".repeat(MAX_PAYLOAD_BYTES)).starts_with("requires_split"));
}

#[test]
fn notebook_version_and_legacy_raw_index_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("n.ipynb");
    std::fs::write(
        &file,
        json!({"cells":[{"cell_type":"code","source":"x=1"}]}).to_string(),
    )
    .unwrap();
    let path = file.to_str().unwrap();
    assert!(validate_notebook(path, &IndexSource::default()).is_err());
    let old = notebook::read(path).unwrap();
    let indexed = IndexSource {
        source_hash: Some(old.source_hash),
        notebook_cells: Some(old.cells),
        chunker_version: Some(notebook::CHUNKER_VERSION),
    };
    assert!(validate_notebook(path, &indexed).is_ok());
    let evidence_before = evidence(path, 1, 1, &indexed);
    assert_eq!(evidence_before.freshness, "verified");
    assert_eq!(evidence_before.cells[0].line_start, 1);
    std::fs::write(
        &file,
        json!({"cells":[{"cell_type":"code","source":"x=2"}]}).to_string(),
    )
    .unwrap();
    assert!(validate_notebook(path, &indexed).is_err());
    assert_eq!(evidence(path, 1, 1, &indexed).freshness, "changed");
    std::fs::remove_file(&file).unwrap();
    let missing = evidence(path, 1, 1, &indexed);
    assert_eq!(missing.origin, "index_only");
    assert_eq!(missing.freshness, "missing");
    assert_eq!(missing.cells[0].cell_index, 0);
}
