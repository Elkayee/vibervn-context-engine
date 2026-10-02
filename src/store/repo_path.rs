//! Normalization helpers for repository paths.
//!
//! Windows paths may be spelled as plain drive paths (`D:\foo`), forward-slash
//! paths (`D:/foo`), extended-length/verbatim paths (`\\?\D:\foo`), repeated
//! separators (`d:\\\\foo`), or UNC / extended-UNC paths (`\\server\share\foo`,
//! `\\?\UNC\server\share\foo`).
//!
//! This module collapses equivalent lexical aliases on Windows to a single
//! canonical identity while preserving UNC prefixes, valid drive-root semantics,
//! verbatim namespaces when meaningful (trailing dots/spaces or verbatim relative),
//! and Unix case-sensitivity.

/// Normalize a repo path to a canonical form for use as a HashMap/gate key.
/// On Windows:
/// - Extended-length absolute drive paths (`\\?\C:\foo`) collapse to plain drive paths (`c:\foo`)
///   unless segments have dot/space suffixes.
/// - Extended UNC paths (`\\?\UNC\server\share\foo`) collapse to ordinary UNC (`\\server\share\foo`)
///   unless segments have dot/space suffixes.
/// - Repeated interior separators collapse to a single `\`.
/// - Trailing separators are stripped (preserving `c:\` drive-root and `c:` drive-relative).
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
    if repo.is_empty() {
        return String::new();
    }

    // Check for extended-length prefix: \\?\ or //?/ (4 chars)
    if is_verbatim_prefix(repo) {
        let after_verbatim = &repo[4..];

        // Case 1: Extended UNC path: \\?\UNC\server\share\... or //?/unc/server/share/...
        if is_unc_prefix(after_verbatim) {
            let unc_body = &after_verbatim[4..];
            let segments: Vec<String> = unc_body
                .split(is_sep)
                .filter(|seg| !seg.is_empty())
                .map(|seg| seg.to_lowercase())
                .collect();

            if has_verbatim_dot_or_space_suffix(&segments) {
                if segments.is_empty() {
                    return r"\\?\unc\".to_string();
                }
                return format!(r"\\?\unc\{}", segments.join("\\"));
            }

            if segments.is_empty() {
                return r"\\".to_string();
            }
            return format!(r"\\{}", segments.join("\\"));
        }

        // Case 2: Extended drive path: \\?\C:\...
        if is_drive_prefix(after_verbatim) {
            let mut chars = after_verbatim.chars();
            let drive_letter = chars.next().unwrap().to_ascii_lowercase();
            chars.next(); // skip ':'
            let after_colon: String = chars.collect();

            let has_leading_sep = after_colon.chars().next().map(is_sep).unwrap_or(false);
            let segments: Vec<String> = after_colon
                .split(is_sep)
                .filter(|seg| !seg.is_empty())
                .map(|seg| seg.to_lowercase())
                .collect();

            // Only supported absolute-drive spelling (with leading separator after colon)
            // and without dot/space suffixes can strip the verbatim prefix.
            if has_leading_sep && !has_verbatim_dot_or_space_suffix(&segments) {
                if segments.is_empty() {
                    return format!("{drive_letter}:\\");
                }
                return format!("{drive_letter}:\\{}", segments.join("\\"));
            }

            // Otherwise, preserve verbatim namespace.
            if has_leading_sep {
                if segments.is_empty() {
                    return format!(r"\\?\{drive_letter}:\");
                }
                return format!(r"\\?\{drive_letter}:\{}", segments.join("\\"));
            } else if segments.is_empty() {
                return format!(r"\\?\{drive_letter}:");
            } else {
                return format!(r"\\?\{drive_letter}:{}", segments.join("\\"));
            }
        }

        // Case 3: Other device namespace (e.g. \\?\Volume{...}\...)
        let segments: Vec<String> = after_verbatim
            .split(is_sep)
            .filter(|seg| !seg.is_empty())
            .map(|seg| seg.to_lowercase())
            .collect();
        if segments.is_empty() {
            return r"\\?\".to_string();
        }
        return format!(r"\\?\{}", segments.join("\\"));
    }

    // Check for device prefix: \\.\ or //./ (4 chars)
    if is_device_prefix(repo) {
        let after_device = &repo[4..];
        let segments: Vec<String> = after_device
            .split(is_sep)
            .filter(|seg| !seg.is_empty())
            .map(|seg| seg.to_lowercase())
            .collect();
        if segments.is_empty() {
            return r"\\.\".to_string();
        }
        return format!(r"\\.\{}", segments.join("\\"));
    }

    // Case 4: Ordinary UNC path: \\server\share\... or //server/share/...
    if is_unc_root(repo) {
        let unc_body = &repo[2..];
        let segments: Vec<String> = unc_body
            .split(is_sep)
            .filter(|seg| !seg.is_empty())
            .map(|seg| seg.to_lowercase())
            .collect();
        if segments.is_empty() {
            return r"\\".to_string();
        }
        return format!(r"\\{}", segments.join("\\"));
    }

    // Case 5: Ordinary drive path: C:\... or C:...
    if is_drive_prefix(repo) {
        let mut chars = repo.chars();
        let drive_letter = chars.next().unwrap().to_ascii_lowercase();
        chars.next(); // skip ':'
        let after_colon: String = chars.collect();

        let has_leading_sep = after_colon.chars().next().map(is_sep).unwrap_or(false);
        let segments: Vec<String> = after_colon
            .split(is_sep)
            .filter(|seg| !seg.is_empty())
            .map(|seg| seg.to_lowercase())
            .collect();

        if has_leading_sep {
            if segments.is_empty() {
                return format!("{drive_letter}:\\");
            }
            return format!("{drive_letter}:\\{}", segments.join("\\"));
        } else if segments.is_empty() {
            return format!("{drive_letter}:");
        } else {
            return format!("{drive_letter}:{}", segments.join("\\"));
        }
    }

    // Case 6: Other relative or root-relative paths
    let is_root_relative = repo.starts_with('\\') || repo.starts_with('/');
    let segments: Vec<String> = repo
        .split(is_sep)
        .filter(|seg| !seg.is_empty())
        .map(|seg| seg.to_lowercase())
        .collect();

    if is_root_relative {
        if segments.is_empty() {
            "\\".to_string()
        } else {
            format!("\\{}", segments.join("\\"))
        }
    } else {
        segments.join("\\")
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

fn has_verbatim_dot_or_space_suffix(segments: &[String]) -> bool {
    segments
        .iter()
        .any(|seg| seg.ends_with(' ') || seg.ends_with('.'))
}
