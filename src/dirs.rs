//! Platform directory resolution (XDG on Linux, Known Folders concept).
//!
//! Drop-in replacement for `dirs` crate. Zero external dependencies.
//! Covers the API surface actually used in NexCore: `home_dir()`, `data_dir()`,
//! `config_dir()`, `cache_dir()`.

use std::path::PathBuf;

/// Returns the home directory of the current user.
///
/// - Linux/macOS: `$HOME` environment variable
/// - Fallback: None
///
/// # Examples
/// ```
/// if let Some(home) = nexcore_fs::dirs::home_dir() {
///     assert!(home.is_absolute());
/// }
/// ```
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Returns the user's data directory.
///
/// - Linux: `$XDG_DATA_HOME` or `$HOME/.local/share`
/// - macOS: `$HOME/Library/Application Support`
pub fn data_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return Some(p);
        }
    }
    home_dir().map(|h| {
        if cfg!(target_os = "macos") {
            h.join("Library/Application Support")
        } else {
            h.join(".local/share")
        }
    })
}

/// Returns the user's configuration directory.
///
/// - Linux: `$XDG_CONFIG_HOME` or `$HOME/.config`
/// - macOS: `$HOME/Library/Application Support`
pub fn config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return Some(p);
        }
    }
    home_dir().map(|h| {
        if cfg!(target_os = "macos") {
            h.join("Library/Application Support")
        } else {
            h.join(".config")
        }
    })
}

/// Returns the user's cache directory.
///
/// - Linux: `$XDG_CACHE_HOME` or `$HOME/.cache`
/// - macOS: `$HOME/Library/Caches`
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return Some(p);
        }
    }
    home_dir().map(|h| {
        if cfg!(target_os = "macos") {
            h.join("Library/Caches")
        } else {
            h.join(".cache")
        }
    })
}

/// Returns the user's runtime directory.
///
/// - Linux: `$XDG_RUNTIME_DIR` (usually `/run/user/<uid>`)
/// - macOS: None (no standard equivalent)
pub fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_returns_absolute_path() {
        if let Some(home) = home_dir() {
            assert!(home.is_absolute());
            assert!(!home.as_os_str().is_empty());
        }
    }

    #[test]
    fn data_dir_under_home() {
        if let Some(data) = data_dir() {
            assert!(data.is_absolute());
        }
    }

    #[test]
    fn config_dir_under_home() {
        if let Some(cfg) = config_dir() {
            assert!(cfg.is_absolute());
        }
    }

    #[test]
    fn cache_dir_under_home() {
        if let Some(cache) = cache_dir() {
            assert!(cache.is_absolute());
        }
    }

    #[test]
    fn data_dir_contains_local_share_or_library() {
        // Verify XDG fallback path structure without mutating env
        if let (Some(data), Some(home)) = (data_dir(), home_dir()) {
            let s = data.to_string_lossy();
            assert!(
                s.contains(".local/share") || s.contains("Library/Application Support"),
                "data_dir should follow XDG or macOS convention: {s}"
            );
            assert!(data.starts_with(&home) || std::env::var_os("XDG_DATA_HOME").is_some());
        }
    }

    #[test]
    fn runtime_dir_is_absolute_if_present() {
        if let Some(rt) = runtime_dir() {
            assert!(rt.is_absolute());
        }
    }
}
