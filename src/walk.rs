//! Recursive directory traversal (replaces `walkdir` crate).
//!
//! Zero external dependencies. Uses `std::fs::read_dir` recursively
//! with configurable depth limits and error handling.

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A directory entry yielded by [`WalkDir`] iteration.
#[derive(Debug, Clone)]
pub struct DirEntry {
    path: PathBuf,
    depth: usize,
    file_type: fs::FileType,
}

impl DirEntry {
    /// Returns the full path to this entry.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Consumes this entry and returns the full path.
    pub fn into_path(self) -> PathBuf {
        self.path
    }

    /// Returns the file type for this entry.
    pub fn file_type(&self) -> fs::FileType {
        self.file_type
    }

    /// Returns the depth of this entry relative to the root.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the file name of this entry.
    pub fn file_name(&self) -> &std::ffi::OsStr {
        self.path
            .file_name()
            .unwrap_or_else(|| self.path.as_os_str())
    }
}

/// Error type for walk operations.
#[derive(Debug)]
pub struct WalkError {
    path: Option<PathBuf>,
    inner: io::Error,
}

impl WalkError {
    /// Returns the path where the error occurred, if available.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the underlying IO error.
    pub fn io_error(&self) -> &io::Error {
        &self.inner
    }

    /// Consumes self and returns the underlying IO error.
    pub fn into_io_error(self) -> io::Error {
        self.inner
    }
}

impl std::fmt::Display for WalkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref p) = self.path {
            write!(f, "walk error at {}: {}", p.display(), self.inner)
        } else {
            write!(f, "walk error: {}", self.inner)
        }
    }
}

impl std::error::Error for WalkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

impl From<WalkError> for io::Error {
    fn from(e: WalkError) -> Self {
        e.inner
    }
}

/// Builder for recursive directory traversal.
///
/// # Examples
/// ```no_run
/// use nexcore_fs::walk::WalkDir;
///
/// for entry in WalkDir::new("/tmp").max_depth(3) {
///     if let Ok(e) = entry {
///         println!("{}", e.path().display());
///     }
/// }
/// ```
pub struct WalkDir {
    root: PathBuf,
    min_depth: usize,
    max_depth: usize,
    follow_links: bool,
    contents_first: bool,
}

impl WalkDir {
    /// Create a new `WalkDir` rooted at the given path.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            min_depth: 0,
            max_depth: usize::MAX,
            follow_links: false,
            contents_first: false,
        }
    }

    /// Set the minimum depth of entries to yield.
    ///
    /// Entries at depths less than `min_depth` are still traversed
    /// but not yielded.
    pub fn min_depth(mut self, depth: usize) -> Self {
        self.min_depth = depth;
        self
    }

    /// Set the maximum depth of entries to yield and traverse.
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Follow symbolic links (default: false).
    pub fn follow_links(mut self, yes: bool) -> Self {
        self.follow_links = yes;
        self
    }

    /// Yield directory contents before the directory itself (default: false).
    pub fn contents_first(mut self, yes: bool) -> Self {
        self.contents_first = yes;
        self
    }
}

impl IntoIterator for WalkDir {
    type Item = Result<DirEntry, WalkError>;
    type IntoIter = WalkDirIter;

    fn into_iter(self) -> WalkDirIter {
        let mut queue = VecDeque::new();
        match fs::metadata(&self.root) {
            Ok(meta) => {
                let entry = DirEntry {
                    path: self.root.clone(),
                    depth: 0,
                    file_type: meta.file_type(),
                };
                queue.push_back(Ok(entry));
            }
            Err(e) => {
                queue.push_back(Err(WalkError {
                    path: Some(self.root.clone()),
                    inner: e,
                }));
            }
        }

        WalkDirIter {
            queue,
            min_depth: self.min_depth,
            max_depth: self.max_depth,
            follow_links: self.follow_links,
            contents_first: self.contents_first,
            deferred_dirs: Vec::new(),
            filter: None,
        }
    }
}

/// Iterator over directory entries.
pub struct WalkDirIter {
    queue: VecDeque<Result<DirEntry, WalkError>>,
    min_depth: usize,
    max_depth: usize,
    follow_links: bool,
    contents_first: bool,
    deferred_dirs: Vec<DirEntry>,
    filter: Option<Box<dyn FnMut(&DirEntry) -> bool>>,
}

impl WalkDirIter {
    /// Skip entries for which the predicate returns false.
    ///
    /// If an entry is a directory and the predicate returns false,
    /// its contents will not be yielded or traversed.
    pub fn filter_entry<F>(mut self, mut predicate: F) -> Self
    where
        F: FnMut(&DirEntry) -> bool + 'static,
    {
        // Apply filter to the already queued root if necessary
        if let Some(Ok(entry)) = self.queue.front() {
            if !predicate(entry) {
                self.queue.pop_front();
            }
        }
        self.filter = Some(Box::new(predicate));
        self
    }

    fn expand_dir(&mut self, entry: &DirEntry) {
        if entry.depth >= self.max_depth {
            return;
        }
        let read = match fs::read_dir(&entry.path) {
            Ok(rd) => rd,
            Err(e) => {
                self.queue.push_back(Err(WalkError {
                    path: Some(entry.path.clone()),
                    inner: e,
                }));
                return;
            }
        };
        for item in read {
            match item {
                Ok(de) => {
                    // TODO: when follow_links=false, use symlink_metadata()
                    let meta_result = de.metadata();
                    match meta_result {
                        Ok(meta) => {
                            let child = DirEntry {
                                path: de.path(),
                                depth: entry.depth.saturating_add(1),
                                file_type: meta.file_type(),
                            };
                            self.queue.push_back(Ok(child));
                        }
                        Err(e) => {
                            self.queue.push_back(Err(WalkError {
                                path: Some(de.path()),
                                inner: e,
                            }));
                        }
                    }
                }
                Err(e) => {
                    self.queue.push_back(Err(WalkError {
                        path: Some(entry.path.clone()),
                        inner: e,
                    }));
                }
            }
        }
    }
}

impl Iterator for WalkDirIter {
    type Item = Result<DirEntry, WalkError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.queue.is_empty() {
                if let Some(dir) = self.deferred_dirs.pop() {
                    if dir.depth >= self.min_depth {
                        return Some(Ok(dir));
                    }
                    continue;
                }
                return None;
            }

            let item = self.queue.pop_front()?;
            match item {
                Err(e) => return Some(Err(e)),
                Ok(entry) => {
                    if let Some(ref mut filter) = self.filter {
                        if !filter(&entry) {
                            continue;
                        }
                    }
                    let is_dir = entry.file_type.is_dir();
                    if is_dir {
                        self.expand_dir(&entry);
                    }
                    if entry.depth < self.min_depth {
                        continue;
                    }
                    if self.contents_first && is_dir {
                        self.deferred_dirs.push(entry);
                        continue;
                    }
                    return Some(Ok(entry));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create isolated test tree with unique name to avoid parallel test conflicts.
    fn setup_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexcore-fs-walk-{name}"));
        // Best-effort cleanup of prior runs
        #[allow(unused_must_use)]
        {
            fs::remove_dir_all(&dir);
        }
        fs::create_dir_all(dir.join("a/b/c")).ok();
        fs::write(dir.join("a/file1.txt"), "hello").ok();
        fs::write(dir.join("a/b/file2.txt"), "world").ok();
        fs::write(dir.join("a/b/c/file3.txt"), "deep").ok();
        dir
    }

    fn teardown(root: &Path) {
        #[allow(unused_must_use)]
        {
            fs::remove_dir_all(root);
        }
    }

    #[test]
    fn walks_all_entries() {
        let root = setup_tree("all");
        let entries: Vec<_> = WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();
        // root + a + a/b + a/b/c + file1 + file2 + file3
        assert!(entries.len() >= 7, "got {} entries", entries.len());
        teardown(&root);
    }

    #[test]
    fn min_depth_filters() {
        let root = setup_tree("min");
        let entries: Vec<_> = WalkDir::new(&root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();
        for e in &entries {
            assert_ne!(e.path(), root.as_path());
        }
        teardown(&root);
    }

    #[test]
    fn max_depth_limits() {
        let root = setup_tree("max");
        let entries: Vec<_> = WalkDir::new(&root)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();
        for e in &entries {
            assert!(e.depth() <= 1, "depth {} > 1", e.depth());
        }
        teardown(&root);
    }

    #[test]
    fn nonexistent_root_yields_error() {
        let entries: Vec<_> = WalkDir::new("/nonexistent/path/xyz").into_iter().collect();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_err());
    }

    #[test]
    fn dir_entry_file_name() {
        let root = setup_tree("fname");
        let files: Vec<_> = WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_type().is_dir())
            .collect();
        let names: Vec<_> = files
            .iter()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"file1.txt".to_string()));
        assert!(names.contains(&"file2.txt".to_string()));
        assert!(names.contains(&"file3.txt".to_string()));
        teardown(&root);
    }
}
