use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;

use reqwest::Client;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;

use context_engine_rs::config::{
    CURRENT_VERSION, Settings, config_path, ensure_dir_and_load, write_settings_atomic,
};
use context_engine_rs::router::{RouterBootOptions, build_router_app};
use context_engine_rs::store::{db_path, normalize_repo_path, sanitize_repo_name};

#[test]
fn test_normalize_repo_path_windows_drive_aliases() {
    if !cfg!(windows) {
        return;
    }

    let expected = r"d:\rubiksolver";
    let variants = [
        r"D:\rubiksolver",
        r"d:\rubiksolver",
        r"D:/rubiksolver",
        r"d:\rubiksolver\",
        r"D:/rubiksolver/",
        r"d:\\\\rubiksolver",
        r"d:\/\rubiksolver",
        r"\\?\D:\rubiksolver",
        r"\\?\d:\rubiksolver",
        r"//?/d:/rubiksolver",
        r"\\?\D:\rubiksolver\",
        r"\\?\d:\\rubiksolver",
    ];

    for variant in variants {
        assert_eq!(
            normalize_repo_path(variant),
            expected,
            "variant '{variant}' must normalize to '{expected}'"
        );
    }

    let expected_chatcmd = r"c:\tools\chatcmd";
    assert_eq!(normalize_repo_path(r"c:\tools\chatcmd"), expected_chatcmd);
    assert_eq!(
        normalize_repo_path(r"\\?\c:\tools\chatcmd"),
        expected_chatcmd
    );

    let expected_all_data = r"d:\xldl_vu\all_data";
    assert_eq!(
        normalize_repo_path(r"d:\xldl_vu\all_data"),
        expected_all_data
    );
    assert_eq!(
        normalize_repo_path(r"\\?\d:\xldl_vu\all_data"),
        expected_all_data
    );
    assert_eq!(
        normalize_repo_path(r"D:/XLDL_VU/all_data/"),
        expected_all_data
    );
}

#[test]
fn test_normalize_repo_path_unc_and_extended_unc() {
    if !cfg!(windows) {
        return;
    }

    let expected = r"\\server\share\repo";
    let unc_variants = [
        r"\\server\share\repo",
        r"//server/share/repo",
        r"\\server\share\repo\",
        r"\\server\\share\\\repo",
        r"\\server\share\repo\/",
        r"\\?\UNC\server\share\repo",
        r"//?/unc/server/share/repo",
        r"\\?\unc\server\\share\repo\",
        r"\\?\UNC\SERVER\SHARE\REPO",
    ];

    for variant in unc_variants {
        let normalized = normalize_repo_path(variant);
        assert_eq!(
            normalized, expected,
            "UNC variant '{variant}' must normalize to '{expected}'"
        );
        assert!(
            normalized.starts_with(r"\\"),
            "UNC path must preserve leading double separator"
        );
        assert!(
            !normalized.starts_with(r"\\\"),
            "UNC path must not have triple leading separator"
        );
    }

    let unc_root_expected = r"\\server\share";
    assert_eq!(normalize_repo_path(r"\\server\share"), unc_root_expected);
    assert_eq!(normalize_repo_path(r"\\server\share\"), unc_root_expected);
    assert_eq!(
        normalize_repo_path(r"\\?\UNC\server\share"),
        unc_root_expected
    );
}

#[test]
fn test_normalize_repo_path_drive_roots() {
    if !cfg!(windows) {
        return;
    }

    assert_eq!(normalize_repo_path(r"C:\"), r"c:\");
    assert_eq!(normalize_repo_path(r"c:/"), r"c:\");
    assert_eq!(normalize_repo_path(r"C:\\"), r"c:\");
    assert_eq!(normalize_repo_path(r"\\?\c:\"), r"c:\");
    assert_eq!(normalize_repo_path(r"C:"), r"c:");
    assert_eq!(normalize_repo_path(r"c:"), r"c:");
}

#[test]
fn test_normalize_repo_path_device_paths() {
    if !cfg!(windows) {
        return;
    }

    assert_eq!(normalize_repo_path(r"\\.\pipe\test"), r"\\.\pipe\test");
    assert_eq!(
        normalize_repo_path(r"\\?\Volume{b75e2c83-0000-0000-0000-602200000000}\repo"),
        r"\\?\volume{b75e2c83-0000-0000-0000-602200000000}\repo"
    );
}

#[test]
fn test_normalize_repo_path_drive_relative_and_whitespace() {
    if !cfg!(windows) {
        return;
    }

    assert_eq!(
        normalize_repo_path(r"C:relative\folder"),
        r"c:relative\folder"
    );
    assert_eq!(
        normalize_repo_path(r"C:relative/folder"),
        r"c:relative\folder"
    );
    assert_eq!(normalize_repo_path(r" leading\folder"), r" leading\folder");
    assert_eq!(normalize_repo_path(r" leading/folder/"), r" leading\folder");
}

#[test]
fn test_normalize_repo_path_verbatim_dot_space_and_relative_namespaces() {
    if !cfg!(windows) {
        return;
    }

    assert_eq!(normalize_repo_path(r"\\?\D:\repo "), r"\\?\d:\repo ");
    assert_eq!(normalize_repo_path(r"\\?\D:\repo."), r"\\?\d:\repo.");
    assert_eq!(normalize_repo_path(r"\\?\D:\foo.\bar"), r"\\?\d:\foo.\bar");
    assert_eq!(normalize_repo_path(r"//?/D:/repo /"), r"\\?\d:\repo ");
    assert_eq!(
        normalize_repo_path(r"\\?\UNC\server\share\repo "),
        r"\\?\unc\server\share\repo "
    );
    assert_eq!(
        normalize_repo_path(r"\\?\UNC\server\share\repo."),
        r"\\?\unc\server\share\repo."
    );
    assert_eq!(
        normalize_repo_path(r"\\?\UNC\server\share.\dir"),
        r"\\?\unc\server\share.\dir"
    );
    assert_eq!(
        normalize_repo_path(r"\\?\D:relative\folder"),
        r"\\?\d:relative\folder"
    );
    assert_eq!(normalize_repo_path(r"\\?\D:"), r"\\?\d:");
}

#[test]
fn test_ensure_dir_and_load_deduplicates_settings_and_normalizes_generations() {
    if !cfg!(windows) {
        return;
    }

    let home = TempDir::new().expect("tempdir");
    let path = config_path(home.path());
    fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");

    let raw_settings = r#"{
  "version": 13,
  "repos": [
    "c:\\users\\home33\\ideaprojects\\flightmanagementproject\\src\\main",
    "d:\\rsbot",
    "c:\\users\\home33\\ideaprojects\\local-gitingest-ui",
    "d:\\oasisbot",
    "e:\\torrentdownloads\\grim dawn (2016)\\grim dawn\\grimdawnbotprototype",
    "e:\\1clickvmfull\\volamtruyenky2\\client\\vlauto\\volamsource",
    "c:\\users\\home33\\ideaprojects\\flightmanagementproject",
    "e:\\xldl_vthk",
    "d:\\onedrive downloader",
    "c:\\users\\home33\\ideaprojects\\kiemtra",
    "c:\\tools\\chatcmd",
    "c:\\tools\\hermes-agent",
    "\\\\?\\c:\\tools\\chatcmd",
    "\\\\?\\c:\\tools\\hermes-agent",
    "d:\\rubiksolver",
    "\\\\?\\d:\\rubiksolver",
    "d:\\\\rubiksolver",
    "d:\\xldl_vu\\all_data",
    "\\\\?\\d:\\xldl_vu\\all_data"
  ],
  "embedding": {
    "provider": "voyage",
    "model": "voyage-4-lite",
    "api_keys": ["test-key-123"],
    "embed_concurrency": 64,
    "voyage_base_url": "https://context-engine.viber.vn/v1",
    "dimensions": null
  },
  "llm": {
    "provider": "openai",
    "rerank_model": "vibervn-rag-0626",
    "api_keys": ["test-key-123"],
    "rerank_min_prune_lines": 16,
    "use_structured_output": true,
    "agentic_rag": false,
    "agentic_rag_max_turns": 9,
    "agentic_rag_max_chunk_chars": 50000,
    "agentic_rag_grep_read": true,
    "openai_base_url": "https://context-engine.viber.vn/v1",
    "openai_force_tool_use": false,
    "chat_custom_endpoints": []
  },
  "repo_generations": {
    "c:\\users\\home33\\ideaprojects\\flightmanagementproject\\src": 1,
    "e:\\torrentdownloads\\grim dawn (2016)\\grim dawn\\grimdawnbotprototype": 1,
    "c:\\tools\\chatcmd": 1,
    "\\\\?\\c:\\tools\\chatcmd": 2,
    "c:\\tools\\hermes-agent": 1
  }
}"#;
    fs::write(&path, raw_settings).expect("write seed settings.json");

    let loaded = ensure_dir_and_load(home.path()).expect("load settings");

    assert_eq!(
        loaded.repos.len(),
        14,
        "19 raw entries must collapse to 14 canonical roots"
    );

    let expected_repos = vec![
        r"c:\users\home33\ideaprojects\flightmanagementproject\src\main",
        r"d:\rsbot",
        r"c:\users\home33\ideaprojects\local-gitingest-ui",
        r"d:\oasisbot",
        r"e:\torrentdownloads\grim dawn (2016)\grim dawn\grimdawnbotprototype",
        r"e:\1clickvmfull\volamtruyenky2\client\vlauto\volamsource",
        r"c:\users\home33\ideaprojects\flightmanagementproject",
        r"e:\xldl_vthk",
        r"d:\onedrive downloader",
        r"c:\users\home33\ideaprojects\kiemtra",
        r"c:\tools\chatcmd",
        r"c:\tools\hermes-agent",
        r"d:\rubiksolver",
        r"d:\xldl_vu\all_data",
    ];
    assert_eq!(
        loaded.repos, expected_repos,
        "repos must preserve first logical order"
    );

    assert_eq!(
        loaded.repo_generation(r"c:\tools\chatcmd"),
        2,
        "colliding generation keys must retain the highest counter"
    );
    assert_eq!(
        loaded.repo_generation(r"\\?\c:\tools\chatcmd"),
        2,
        "lookup via alias must return normalized generation"
    );
    assert_eq!(
        loaded.repo_generation(r"c:\users\home33\ideaprojects\flightmanagementproject\src"),
        1,
        "generation entry for root removed from repos must be preserved"
    );

    assert_eq!(loaded.embedding.provider, "voyage");
    assert_eq!(loaded.llm.rerank_model, "vibervn-rag-0626");

    let reloaded = ensure_dir_and_load(home.path()).expect("reload settings");
    assert_eq!(reloaded.repos, loaded.repos);
    assert_eq!(reloaded.repo_generations, loaded.repo_generations);
}

#[test]
fn test_config_cleanup_preserves_indexes_and_sidecars() {
    let home = TempDir::new().expect("tempdir");
    let data_dir = home.path().join("data");
    let sidecar_dir = data_dir.join("sidecar");
    let rocksdb_dir = data_dir.join("rocksdb");
    fs::create_dir_all(&sidecar_dir).expect("create sidecar dir");
    fs::create_dir_all(&rocksdb_dir).expect("create rocksdb dir");

    let canonical_sidecar = sidecar_dir.join("d__rubiksolver.json");
    let legacy_sidecar = sidecar_dir.join("d___rubiksolver.json");
    fs::write(&canonical_sidecar, b"canonical 54 files").expect("write canonical sidecar");
    fs::write(&legacy_sidecar, b"legacy 33 files").expect("write legacy sidecar");

    let path = config_path(home.path());
    fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");
    let raw_settings = r#"{
  "version": 13,
  "repos": [
    "d:\\rubiksolver",
    "\\\\?\\d:\\rubiksolver",
    "d:\\\\rubiksolver"
  ],
  "embedding": {
    "provider": "voyage",
    "model": "voyage-4-lite",
    "api_keys": []
  },
  "llm": {
    "provider": "google",
    "rerank_model": "gemini-3.1-flash-lite",
    "api_keys": []
  }
}"#;
    fs::write(&path, raw_settings).expect("write seed settings.json");

    let loaded = ensure_dir_and_load(home.path()).expect("load settings");
    assert_eq!(loaded.repos, vec![r"d:\rubiksolver"]);

    assert!(
        canonical_sidecar.exists(),
        "canonical sidecar file must remain intact"
    );
    assert_eq!(
        fs::read(&canonical_sidecar).expect("read canonical"),
        b"canonical 54 files"
    );

    assert!(
        legacy_sidecar.exists(),
        "legacy sidecar file must remain intact and not deleted"
    );
    assert_eq!(
        fs::read(&legacy_sidecar).expect("read legacy"),
        b"legacy 33 files"
    );

    assert_eq!(sanitize_repo_name(r"d:\rubiksolver"), "d__rubiksolver");
    assert_eq!(sanitize_repo_name(r"\\?\d:\rubiksolver"), "d__rubiksolver");
    assert_eq!(
        db_path(&data_dir, r"d:\rubiksolver", 0),
        rocksdb_dir.join("d__rubiksolver")
    );
}

async fn start_test_router(home: &TempDir) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let home_path = home.path().to_path_buf();
    let (app, _) = build_router_app(RouterBootOptions {
        data_dir: Some(home_path.join("data")),
        embeddings_dir: Some(home_path.join("embeddings")),
        bind: "127.0.0.1".to_string(),
        home_dir: Some(home_path),
        worker_exe: None,
    })
    .await
    .expect("build router app");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve router");
    });

    addr
}

#[tokio::test]
async fn test_put_config_deduplicates_and_normalizes() {
    if !cfg!(windows) {
        return;
    }

    let home = TempDir::new().expect("tempdir");
    let path = config_path(home.path());
    fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");

    let seed_settings = Settings {
        version: CURRENT_VERSION,
        repos: vec![r"c:\tools\chatcmd".to_string()],
        repo_generations: HashMap::from([(r"c:\tools\chatcmd".to_string(), 3)]),
        ..Settings::default()
    };
    write_settings_atomic(&path, &seed_settings).expect("seed write");

    let addr = start_test_router(&home).await;
    let client = Client::new();

    let put_body = json!({
        "version": CURRENT_VERSION,
        "repos": [
            "c:\\tools\\chatcmd",
            "\\\\?\\c:\\tools\\chatcmd",
            "d:\\rubiksolver",
            "\\\\?\\D:\\RubikSolver",
            "D:/rubiksolver/"
        ],
        "embedding": {
            "provider": "voyage",
            "model": "voyage-4-lite",
            "api_keys": []
        },
        "llm": {
            "provider": "google",
            "rerank_model": "gemini-3.1-flash-lite",
            "api_keys": []
        }
    });

    let resp = client
        .put(format!("http://{addr}/api/config"))
        .json(&put_body)
        .send()
        .await
        .expect("send PUT /api/config");

    assert_eq!(resp.status(), 200);
    let resp_json: serde_json::Value = resp.json().await.expect("parse json");
    let repos = resp_json["repos"]
        .as_array()
        .expect("repos array")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        repos,
        vec![r"c:\tools\chatcmd", r"d:\rubiksolver"],
        "PUT /api/config must normalize and deduplicate repos in first logical order"
    );

    let reloaded = ensure_dir_and_load(home.path()).expect("load persisted config");
    assert_eq!(reloaded.repos, vec![r"c:\tools\chatcmd", r"d:\rubiksolver"]);
    assert_eq!(
        reloaded.repo_generation(r"c:\tools\chatcmd"),
        3,
        "server-owned repo_generations must be preserved on PUT"
    );
}
