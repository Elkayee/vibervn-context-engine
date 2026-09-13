//! Module xu ly phat hien thu muc goc (workspace root) va tu dong dang ky repo cho Viber Context Engine.

use std::path::{Path, PathBuf};
use tracing::info;
use crate::config::{ensure_dir_and_load, write_settings_atomic, config_path, CURRENT_VERSION};
use crate::store::normalize_repo_path;

/// Tim thu muc goc cua du an (root) bang cach duyet nguoc len cac thu muc cha
/// tim cac dau hieu nhan biet (.git, Cargo.toml, package.json, pom.xml, v.v.).
pub fn tim_thu_muc_goc(duong_dan: &Path) -> PathBuf {
    let mut hien_tai = match duong_dan.canonicalize() {
        Ok(p) => p,
        Err(_) => duong_dan.to_path_buf(),
    };
    if hien_tai.is_file() {
        if let Some(cha) = hien_tai.parent() {
            hien_tai = cha.to_path_buf();
        }
    }

    let dau_hieu = [
        ".git",
        "Cargo.toml",
        "package.json",
        "pom.xml",
        "build.gradle",
        "go.mod",
        "pyproject.toml",
        "requirements.txt",
        "composer.json",
    ];

    let mut con_tro = hien_tai.clone();
    loop {
        for dh in &dau_hieu {
            if con_tro.join(dh).exists() {
                return con_tro;
            }
        }
        match con_tro.parent() {
            Some(cha) if cha != con_tro => {
                con_tro = cha.to_path_buf();
            }
            _ => break,
        }
    }
    hien_tai
}

/// Tu dong xac dinh kho luu tru (repository) phu hop nhat tu cac goi y:
/// 1. Tham so workspace truyen truc tiep
/// 2. Duong dan tep (file_path)
/// 3. Runtime state tu Agent (active_workspace.txt)
/// 4. Trich xuat duong dan tu noi dung truy van
/// 5. Thu muc lam viec hien tai (current_dir)
/// 6. So khop ten kho da cau hinh
/// 7. Fallback an toan vao kho dau tien
pub fn tim_kho_tu_dong(
    goi_y_kho: Option<&str>,
    goi_y_tep: Option<&str>,
    goi_y_truy_van: Option<&str>,
    cac_kho_cau_hinh: &[String],
) -> Option<String> {
    // 1. Goi y kho truc tiep
    if let Some(k) = goi_y_kho.map(str::trim).filter(|s| !s.is_empty()) {
        let p = Path::new(k);
        let goc = tim_thu_muc_goc(p);
        return Some(normalize_repo_path(&goc.to_string_lossy()));
    }

    // 2. Goi y tu duong dan tep
    if let Some(t) = goi_y_tep.map(str::trim).filter(|s| !s.is_empty()) {
        let p = Path::new(t);
        let duong_dan_tep = if p.is_absolute() {
            p.to_path_buf()
        } else if let Ok(cwd) = std::env::current_dir() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };
        let goc = tim_thu_muc_goc(&duong_dan_tep);
        return Some(normalize_repo_path(&goc.to_string_lossy()));
    }

    // 3. Runtime state tu AGY / Agent (active_workspace.txt hoac active_repo.txt)
    let home_nd = dirs::home_dir();
    if let Some(ref hm) = home_nd {
        let cac_tep_trang_thai = [
            hm.join(".gemini").join("active_workspace.txt"),
            PathBuf::from(r"C:\Tools\hermes-agent\.agents\active_repo.txt"),
        ];
        for tep_ws in &cac_tep_trang_thai {
            if tep_ws.is_file() {
                if let Ok(nd) = std::fs::read_to_string(tep_ws) {
                    let nd_trim = nd.trim();
                    let p = Path::new(nd_trim);
                    if p.exists() {
                        let goc = tim_thu_muc_goc(p);
                        return Some(normalize_repo_path(&goc.to_string_lossy()));
                    }
                }
            }
        }
    }

    // 4. Trich xuat duong dan tu noi dung truy van
    if let Some(tv) = goi_y_truy_van {
        for tu in tv.split_whitespace() {
            let tu_sach = tu.trim_matches(|c| c == '\'' || c == '"' || c == '(' || c == ')' || c == '[' || c == ']');
            if (tu_sach.len() >= 3 && tu_sach.chars().nth(1) == Some(':') && tu_sach.chars().nth(2) == Some('\\'))
                || tu_sach.starts_with('/')
            {
                let p = Path::new(tu_sach);
                if p.exists() {
                    let goc = tim_thu_muc_goc(p);
                    return Some(normalize_repo_path(&goc.to_string_lossy()));
                }
            }
        }
    }

    // 5. Kiem tra thu muc CWD hien tai
    if let Ok(cwd) = std::env::current_dir() {
        let la_home = home_nd.as_ref().is_some_and(|h| h == &cwd);
        if !la_home {
            let goc = tim_thu_muc_goc(&cwd);
            if goc.join(".git").exists() || goc.join("Cargo.toml").exists() || goc.join("package.json").exists() {
                return Some(normalize_repo_path(&goc.to_string_lossy()));
            }
        }
    }

    // 6. So khop tuong doi voi cac kho da cau hinh
    if let Some(tv) = goi_y_truy_van {
        let tv_thuong = tv.to_lowercase();
        for k in cac_kho_cau_hinh {
            let ten_kho = Path::new(k)
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !ten_kho.is_empty() && tv_thuong.contains(&ten_kho) {
                return Some(normalize_repo_path(k));
            }
        }
    }

    // 7. Fallback vao cac repo quen thuoc hoac repo dau tien
    if let Some(k) = cac_kho_cau_hinh.iter().find(|k| k.to_lowercase().contains("chatcmd")) {
        return Some(normalize_repo_path(k));
    }
    if let Some(k) = cac_kho_cau_hinh.iter().find(|k| k.to_lowercase().contains("hermes")) {
        return Some(normalize_repo_path(k));
    }
    if let Some(k) = cac_kho_cau_hinh.first() {
        return Some(normalize_repo_path(k));
    }

    // Cuoi cung: fallback vao CWD
    if let Ok(cwd) = std::env::current_dir() {
        return Some(normalize_repo_path(&cwd.to_string_lossy()));
    }

    None
}

/// Dam bao mot repo da duoc dang ky vao settings.json cua he thong.
/// Neu chua co, tu dong them vao danh sach repos va ghi lai file settings.json.
pub async fn dam_bao_kho_duoc_dang_ky(duong_dan_home: &Path, tm_kho: &str) -> anyhow::Result<bool> {
    let kho_chuan = normalize_repo_path(tm_kho);
    if kho_chuan.is_empty() {
        return Ok(false);
    }
    let p = Path::new(&kho_chuan);
    if !p.is_dir() {
        return Ok(false);
    }

    let home = duong_dan_home.to_path_buf();
    let kho = kho_chuan.clone();
    tokio::task::spawn_blocking(move || {
        let mut settings = ensure_dir_and_load(&home)
            .map_err(|e| anyhow::anyhow!("Loi doc settings: {e}"))?;
        if !settings.repos.iter().any(|k| normalize_repo_path(k) == kho) {
            info!(repo = %kho, "Tu dong dang ky repo moi vao settings.json");
            settings.repos.push(kho.clone());
            settings.version = CURRENT_VERSION;
            let dich = config_path(&home);
            write_settings_atomic(&dich, &settings)
                .map_err(|e| anyhow::anyhow!("Loi ghi settings: {e}"))?;
            return Ok(true);
        }
        Ok(false)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tim_thu_muc_goc() {
        let temp = tempfile::tempdir().unwrap();
        let sub = temp.path().join("src").join("module");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "").unwrap();

        let goc = tim_thu_muc_goc(&sub);
        assert_eq!(goc.canonicalize().unwrap(), temp.path().canonicalize().unwrap());
    }

    #[test]
    fn test_tim_kho_tu_dong_explicit() {
        let cac_kho = vec![r"C:\Tools\chatcmd".to_string(), r"C:\Tools\hermes-agent".to_string()];
        let ket_qua = tim_kho_tu_dong(Some(r"C:\Tools\chatcmd"), None, None, &cac_kho);
        assert!(ket_qua.is_some());
        assert!(ket_qua.unwrap().to_lowercase().contains("chatcmd"));
    }
}
