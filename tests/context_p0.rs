use context_engine_rs::{
    config::Settings,
    mcp::run_file_retrieval,
    parsing::{notebook, parse_file},
    query::{
        context,
        merger::{MergeChunk, merge_chunks},
    },
    store::{
        self,
        ops::{FileMeta, upsert_file_meta},
    },
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
};

#[tokio::test]
async fn stored_notebook_retrieval_is_code_only_and_rejects_legacy_before_egress() {
    let temp = tempfile::tempdir().unwrap();
    let repo_dir = temp.path().join("repo");
    std::fs::create_dir(&repo_dir).unwrap();
    let file = repo_dir.join("analysis.ipynb");
    let raw = json!({"cells":[
        {"cell_type":"markdown","source":"MARKDOWN_SENTINEL"},
        {"cell_type":"code","id":"answer-cell","source":["answer = 42\n","print(answer)\n"],
         "outputs":[{"text":"OUTPUT_SENTINEL","data":{"image/png":"BASE64_SENTINEL"}}]},
        {"cell_type":"raw","source":"RAW_SENTINEL"}
    ]})
    .to_string();
    std::fs::write(&file, &raw).unwrap();
    let repo = store::normalize_repo_path(repo_dir.to_str().unwrap());
    let file = std::path::PathBuf::from(&repo).join("analysis.ipynb");
    let parsed = parse_file(file.to_str().unwrap(), &raw);
    let data = temp.path().join("data");
    let db = store::open_db(&data, &repo, 0).await.unwrap();
    for chunk in &parsed.chunks {
        db.query("CREATE chunk SET file = $file, line_start = $start, line_end = $end, content = $content, embedding = $embedding")
            .bind(("file", chunk.file.clone())).bind(("start", chunk.line_start as i64))
            .bind(("end", chunk.line_end as i64)).bind(("content", chunk.content.clone()))
            .bind(("embedding", vec![1.0f32; 1024])).await.unwrap().check().unwrap();
    }
    let mut meta = FileMeta {
        path: file.to_string_lossy().into(),
        mtime: 1,
        size: raw.len() as i64,
        repo: repo.clone(),
        chunk_count: parsed.chunks.len() as i64,
        chunker_version: 2,
        source_hash: parsed.source_hash.clone(),
        notebook_cells: Some(parsed.notebook_cells.clone()),
    };
    upsert_file_meta(&db, &meta).await.unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let counter = calls.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 8192];
        let read = socket.read(&mut bytes).await.unwrap();
        let request = String::from_utf8_lossy(&bytes[..read]);
        for marker in [
            "OUTPUT_SENTINEL",
            "BASE64_SENTINEL",
            "MARKDOWN_SENTINEL",
            "RAW_SENTINEL",
        ] {
            assert!(!request.contains(marker));
        }
        counter.fetch_add(1, Ordering::SeqCst);
        let body = json!({"data":[{"embedding":vec![1.0f32;1024]}],"usage":{"total_tokens":1}})
            .to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let mut settings = Settings::default();
    settings.embedding.provider = "voyage".into();
    settings.embedding.model = "voyage-code-3".into();
    settings.embedding.api_keys = vec!["fixture-key".into()];
    settings.embedding.voyage_base_url = Some(format!("http://{addr}/v1"));
    settings.llm.api_keys.clear();
    let dbs = Arc::new(RwLock::new(HashMap::from([(repo.clone(), db.clone())])));
    let legacy = run_file_retrieval(
        &data,
        &dbs,
        &settings,
        &repo,
        "analysis.ipynb",
        "find answer",
        5,
    )
    .await;
    assert!(legacy.contains("reindex"), "{legacy}");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    meta.chunker_version = notebook::CHUNKER_VERSION;
    upsert_file_meta(&db, &meta).await.unwrap();
    let saved = context::indexed_source(&db, file.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(saved.source_hash, parsed.source_hash);
    assert_eq!(saved.chunker_version, Some(notebook::CHUNKER_VERSION));
    assert_eq!(
        saved.notebook_cells.as_ref().unwrap(),
        &parsed.notebook_cells
    );
    let output = run_file_retrieval(
        &data,
        &dbs,
        &settings,
        &repo,
        "analysis.ipynb",
        "find answer",
        5,
    )
    .await;
    assert!(output.contains("answer = 42"), "{output}");
    assert!(output.contains("answer-cell"), "{output}");
    assert!(
        output.contains(parsed.source_hash.as_deref().unwrap()),
        "{output}"
    );
    for marker in [
        "OUTPUT_SENTINEL",
        "BASE64_SENTINEL",
        "MARKDOWN_SENTINEL",
        "RAW_SENTINEL",
    ] {
        assert!(
            !output.contains(marker),
            "non-code content leaked: {marker}"
        );
    }
    assert!(context::mcp_text_bytes(&output) <= context::MAX_PAYLOAD_BYTES);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    std::fs::write(
        &file,
        json!({"cells":[{"cell_type":"code","source":"answer=43"}]}).to_string(),
    )
    .unwrap();
    let changed = run_file_retrieval(
        &data,
        &dbs,
        &settings,
        &repo,
        "analysis.ipynb",
        "find answer",
        5,
    )
    .await;
    assert!(changed.contains("version changed"), "{changed}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

fn chunk(file: &str, start: u32, text: &str) -> MergeChunk {
    MergeChunk {
        file: file.into(),
        line_start: start,
        line_end: start,
        score: 1.0,
        content: text.into(),
        symbol: None,
        symbol_fqn: None,
        symbol_kind: None,
    }
}

#[test]
fn tied_scores_have_stable_order_and_notebook_cells_do_not_merge() {
    let a = merge_chunks(vec![chunk("b.py", 1, "b"), chunk("a.py", 1, "a")], 10);
    let b = merge_chunks(vec![chunk("a.py", 1, "a"), chunk("b.py", 1, "b")], 10);
    assert_eq!(
        a.iter().map(|c| &c.file).collect::<Vec<_>>(),
        b.iter().map(|c| &c.file).collect::<Vec<_>>()
    );
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("n.ipynb");
    std::fs::write(
        &file,
        json!({"cells":[
            {"cell_type":"code","source":"a=1"},{"cell_type":"code","source":"b=2"}
        ]})
        .to_string(),
    )
    .unwrap();
    let result = merge_chunks(
        vec![
            chunk(file.to_str().unwrap(), 1, "a=1"),
            chunk(file.to_str().unwrap(), 3, "b=2"),
        ],
        10,
    );
    assert_eq!(
        result.len(),
        2,
        "different cells must keep separate source ranges"
    );
}

#[test]
fn final_evidence_is_bounded_without_destroying_console_diagnostics() {
    use context_engine_rs::query::engine::{CodeResult, QueryResult, QueryTiming, RerankInfo};
    let result = |file: &str| CodeResult {
        source: None,
        file: file.into(),
        line_start: 1,
        line_end: 1,
        score: 1.0,
        content: "code\n".repeat(10_000),
        symbol: None,
        callers: None,
        caller_files: None,
        caller_names: vec![],
        callee_names: vec![],
        callees: None,
    };
    let mut query = QueryResult {
        budget: Default::default(),
        warnings: vec![],
        results: vec![result("a.py"), result("b.py")],
        pre_rerank_results: vec![result("diagnostic.py")],
        timing: QueryTiming {
            embed_ms: 0,
            search_ms: 0,
            graph_ms: 0,
            merge_ms: 0,
            rerank_ms: 0,
            total_ms: 0,
        },
        rerank: Some(RerankInfo {
            raw_request: "DIAGNOSTIC_REQUEST".repeat(10_000),
            raw_response: "DIAGNOSTIC_RESPONSE".into(),
            fallback_used: false,
            skip_reason: None,
        }),
        warming: false,
        graph_pending: false,
    };
    let diagnostic_len = query.rerank.as_ref().unwrap().raw_request.len();
    context::bound_query(&mut query);
    assert!(query.budget.truncated);
    assert!(query.budget.serialized_bytes.unwrap() <= context::MAX_PAYLOAD_BYTES);
    assert!(!query.warnings.is_empty());
    assert_eq!(query.pre_rerank_results.len(), 1);
    assert_eq!(
        query.rerank.as_ref().unwrap().raw_request.len(),
        diagnostic_len
    );
    assert_eq!(query.budget.scope, "final_evidence");
    assert!(!query.budget.token_limit_verified);
}
