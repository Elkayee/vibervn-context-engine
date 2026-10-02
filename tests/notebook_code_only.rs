use context_engine_rs::{indexing::walker::has_indexable_extension, parsing::parse_file};
use serde_json::json;
use std::path::Path;

fn notebook() -> String {
    json!({
        "nbformat": 4,
        "nbformat_minor": 5,
        "metadata": {"private": "METADATA_SENTINEL"},
        "cells": [
            {"cell_type": "markdown", "source": ["MARKDOWN_SENTINEL"]},
            {"cell_type": "code", "id": "code-a", "source": ["answer = 42\n", "print(answer)\n"],
             "execution_count": 17,
             "outputs": [{"output_type": "stream", "text": "OUTPUT_SENTINEL"},
                         {"data": {"image/png": "BASE64_SENTINEL"}}]},
            {"cell_type": "raw", "source": "RAW_SENTINEL"},
            {"cell_type": "code", "source": "greeting = 'chào bạn'\n"}
        ]
    })
    .to_string()
}

#[test]
fn notebook_is_indexable_without_custom_extensions() {
    assert!(has_indexable_extension(Path::new("analysis.ipynb")));
}

#[test]
fn notebook_index_chunks_contain_code_and_exclude_non_code() {
    let result = parse_file("analysis.ipynb", &notebook());
    let content = result
        .chunks
        .iter()
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        content.contains("answer = 42\nprint(answer)"),
        "source arrays must concatenate into code"
    );
    assert!(content.contains("greeting = 'chào bạn'"));
    for marker in [
        "OUTPUT_SENTINEL",
        "BASE64_SENTINEL",
        "MARKDOWN_SENTINEL",
        "RAW_SENTINEL",
        "METADATA_SENTINEL",
    ] {
        assert!(
            !content.contains(marker),
            "non-code content leaked: {marker}"
        );
    }
}

#[test]
fn notebook_without_code_produces_no_index_chunks() {
    let raw = json!({"cells": [{"cell_type": "markdown", "source": "not executable"}]}).to_string();
    assert!(parse_file("empty.ipynb", &raw).chunks.is_empty());
}

#[test]
fn malformed_notebook_is_reported_as_an_error() {
    let malformed = parse_file("bad.ipynb", "{bad JSON");
    assert!(
        malformed
            .error
            .as_deref()
            .unwrap()
            .contains("invalid notebook")
    );
    assert!(malformed.chunks.is_empty());
    assert!(parse_file("empty.ipynb", r#"{"cells":[]}"#).error.is_none());
}

#[test]
fn read_and_grep_use_code_projection_instead_of_json_lines() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("analysis.ipynb"), notebook()).unwrap();
    let read =
        context_engine_rs::fs_tools::run_read(tmp.path(), "analysis.ipynb", Some(1), Some(2));
    assert!(read.ok);
    assert!(read.text.contains("1: answer = 42"));
    assert!(read.text.contains("2: print(answer)"));
    for marker in [
        "OUTPUT_SENTINEL",
        "BASE64_SENTINEL",
        "MARKDOWN_SENTINEL",
        "RAW_SENTINEL",
    ] {
        assert!(!read.text.contains(marker));
        let grep = context_engine_rs::fs_tools::run_grep(
            tmp.path(),
            marker,
            Some("*.ipynb"),
            true,
            false,
            0,
        );
        assert!(grep.ok);
        assert!(grep.regions.is_empty(), "non-code match leaked: {marker}");
    }
    let grep = context_engine_rs::fs_tools::run_grep(
        tmp.path(),
        "answer = 42",
        Some("*.ipynb"),
        true,
        false,
        0,
    );
    assert!(grep.text.contains("answer = 42"));
}
