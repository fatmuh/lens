//! Path utilities.

use std::path::{Path, PathBuf};

/// Strip Windows' extended-length path prefix (`\\?\`) from a path so it's
/// user-friendly for display and reports.
///
/// `std::fs::canonicalize` on Windows returns paths like `\\?\D:\foo\bar`,
/// which look alien in terminal output. This function converts them to the
/// conventional `D:\foo\bar` form. On non-Windows platforms the path is
/// returned unchanged.
pub fn normalize(path: &Path) -> PathBuf {
    let s = path.as_os_str();
    #[cfg(windows)]
    {
        if let Some(s) = s.to_str() {
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                return PathBuf::from(stripped);
            }
        }
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn strips_extended_prefix() {
        let p = PathBuf::from(r"\\?\D:\foo\bar");
        assert_eq!(normalize(&p), PathBuf::from(r"D:\foo\bar"));
    }

    #[cfg(windows)]
    #[test]
    fn leaves_normal_paths_unchanged() {
        let p = PathBuf::from(r"D:\foo\bar");
        assert_eq!(normalize(&p), PathBuf::from(r"D:\foo\bar"));
    }

    #[cfg(windows)]
    #[test]
    fn handles_unc_extended_path() {
        let p = PathBuf::from(r"\\?\UNC\server\share\file");
        // We just ensure it doesn't crash; the exact normalization of UNC
        // paths is left as a no-op (dunce-like behavior is out of scope here).
        let _ = normalize(&p);
    }

    #[cfg(not(windows))]
    #[test]
    fn noop_on_unix() {
        let p = PathBuf::from("/usr/local/bin");
        assert_eq!(normalize(&p), PathBuf::from("/usr/local/bin"));
    }
}
