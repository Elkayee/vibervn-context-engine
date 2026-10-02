//! Normalization helpers for repository paths.
//!
//! Windows paths may be spelled as plain drive paths (`D:\foo`), forward-slash
//! paths (`D:/foo`), extended-length/verbatim paths (`\\?\D:\foo`), repeated
//! separators (`d:\\\\foo`), or UNC / extended-UNC paths (`\\server\share\foo`,
//! `\\?\UNC\server\share\foo`).
//!
//! This module collapses equivalent lexical aliases on Windows to a single
//! canonical identity while preserving UNC prefixes, valid drive-root semantics,
//! and Unix case-sensitivity.

/// Normalize a repo path to a canonical form for use as a HashMap/gate key.
/// On Windows:
/// - Extended-length drive paths (`\\?\C:\foo`) collapse to plain drive paths (`c:\foo`).
/// - Extended UNC paths (`\\?\UNC\server\share\foo`) collapse to ordinary UNC (`\\server\share\foo`).
/// - Repeated interior separators collapse to a single `\`.
/// - Trailing separators are stripped (preserving `c:\` drive-root).
/// - Lowercased (NTFS is case-insensitive).
/// On Unix: forward slashes only, trailing separators stripped (case-sensitive).
pub fn normalize_repo_path(repo: &str) -> String {
    if !cfg!(windows) {
        let s = repo.replace('\\', "/");
        return s.trim_end_matches(['/', '\\']).to_string();
    }

    normalize_windows_path(repo)
}

fn normalize_windows_path(repo: &str) -> String {
    let raw = repo.trim();
    if raw.is_empty() {
        return String::new();
    }

    // Check for extended-length prefix: \\?\ or //?/ (4 chars)
    if is_verbatim_prefix(raw) {
        let after_verbatim = &raw[4..];

        // Case 1: Extended UNC path: \\?\UNC\server\share\...
        if is_unc_prefix(after_verbatim) {
            let unc_body = &after_verbatim[4..];
            return format_unc_path(unc_body);
        }

        // Case 2: Extended drive path: \\?\C:\...
        if is_drive_prefix(after_verbatim) {
            return format_drive_path(after_verbatim);
        }

        // Case 3: Other device namespace (e.g. \\?\Volume{...}\...)
        // Keep \\?\ prefix, normalize interior slashes and lowercase.
        let body = normalize_slashes_and_segments(after_verbatim);
        if body.is_empty() {
            return r"\\?\".to_string();
        }
        return format!(r"\\?\{body}");
    }

    // Check for device prefix: \\.\ or //./ (4 chars)
    if is_device_prefix(raw) {
        let after_device = &raw[4..];
        let body = normalize_slashes_and_segments(after_device);
        if body.is_empty() {
            return r"\\.\".to_string();
        }
        return format!(r"\\.\{body}");
    }

    // Case 4: Ordinary UNC path: \\server\share\... or //server/share/...
    if is_unc_root(raw) {
        let unc_body = &raw[2..];
        return format_unc_path(unc_body);
    }

    // Case 5: Ordinary drive path: C:\... or C:...
    if is_drive_prefix(raw) {
        return format_drive_path(raw);
    }

    // Case 6: Other relative or root-relative paths
    let is_root_relative = raw.starts_with('\\') || raw.starts_with('/');
    let body = normalize_slashes_and_segments(raw);
    if is_root_relative {
        if body.is_empty() {
            "\\".to_string()
        } else {
            format!("\\{body}")
        }
    } else {
        body
    }
}

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

fn is_verbatim_prefix(s: &str) -> bool {
    let chars: Vec<char> = s.chars().take(4).collect();
    chars.len() == 4 && is_sep(chars[0]) && is_sep(chars[1]) && chars[2] == '?' && is_sep(chars[3])
}

fn is_device_prefix(s: &str) -> bool {
    let chars: Vec<char> = s.chars().take(4).collect();
    chars.len() == 4 && is_sep(chars[0]) && is_sep(chars[1]) && chars[2] == '.' && is_sep(chars[3])
}

fn is_unc_prefix(s: &str) -> bool {
    let chars: Vec<char> = s.chars().take(4).collect();
    chars.len() == 4
        && chars[0].eq_ignore_ascii_case(&'u')
        && chars[1].eq_ignore_ascii_case(&'n')
        && chars[2].eq_ignore_ascii_case(&'c')
        && is_sep(chars[3])
}

fn is_unc_root(s: &str) -> bool {
    let chars: Vec<char> = s.chars().take(3).collect();
    chars.len() >= 2
        && is_sep(chars[0])
        && is_sep(chars[1])
        && (chars.len() == 2 || !is_sep(chars[2]))
}

fn is_drive_prefix(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(second) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic() && second == ':'
}

fn format_unc_path(unc_body: &str) -> String {
    let segments: Vec<String> = unc_body
        .split(|c| is_sep(c))
        .filter(|seg| !seg.is_empty())
        .map(|seg| seg.to_lowercase())
        .collect();

    if segments.is_empty() {
        r"\\".to_string()
    } else {
        format!(r"\\{}", segments.join("\\"))
    }
}

fn format_drive_path(drive_str: &str) -> String {
    let mut chars = drive_str.chars();
    let drive_letter = chars.next().unwrap().to_ascii_lowercase();
    chars.next(); // skip ':'
    let after_colon: String = chars.collect();

    let has_leading_sep = after_colon.chars().next().map(is_sep).unwrap_or(false);
    let segments: Vec<String> = after_colon
        .split(|c| is_sep(c))
        .filter(|seg| !seg.is_empty())
        .map(|seg| seg.to_lowercase())
        .collect();

    if segments.is_empty() {
        if has_leading_sep {
            format!("{drive_letter}:\\")
        } else {
            format!("{drive_letter}:")
        }
    } else {
        format!("{drive_letter}:\\{}", segments.join("\\"))
    }
}

fn normalize_slashes_and_segments(s: &str) -> String {
    let segments: Vec<String> = s
        .split(|c| is_sep(c))
        .filter(|seg| !seg.is_empty())
        .map(|seg| seg.to_lowercase())
        .collect();
    segments.join("\\")
}
