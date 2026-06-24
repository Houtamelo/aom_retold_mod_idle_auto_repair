//! Persistent cache for expensive engine-data extraction and per-file parse
//! results.
//!
//! Layout under the cache root (`~/.local/state/aomr_lsp/` or the fallback
//! `~/.aomr_lsp/`):
//!
//! ```text
//! v1/<sha256-of-doxygen_retail.7z>.json          engine API (syscalls + aiplans)
//! game_parse/v1/<mtime>-<sha256>.json            per-file parse tree + symbols
//! ```
//!
//! The `v1/` prefix is a schema-version marker. Future incompatible schema
//! changes can write to `v2/` without invalidating older cache files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Returns the user-level cache root directory for this LSP.
///
/// Uses `dirs::state_dir()` on platforms that provide it (`~/.local/state` on
/// Linux), falling back to `~/.aomr_lsp/` when not available.
pub fn state_cache_dir() -> PathBuf {
    dirs::state_dir()
        .map(|d| d.join("aomr_lsp"))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .expect("home directory is required for cache fallback")
                .join(".aomr_lsp")
        })
}

/// `v1/` subdirectory for engine API cache files.
pub fn engine_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("v1")
}

/// `game_parse/v1/` subdirectory for per-file parse caches.
pub fn parse_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("game_parse").join("v1")
}

/// Create all cache subdirectories if they do not already exist.
pub fn ensure_cache_dirs(cache_dir: &Path) -> Result<()> {
    fs::create_dir_all(engine_cache_dir(cache_dir))
        .with_context(|| format!("creating engine cache dir under {cache_dir:?}"))?;
    fs::create_dir_all(parse_cache_dir(cache_dir))
        .with_context(|| format!("creating parse cache dir under {cache_dir:?}"))?;
    Ok(())
}

/// Compute the lowercase hex SHA-256 of the file at `path`.
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .with_context(|| format!("reading file for SHA-256: {path:?}"))?;
    Ok(sha256_bytes(&bytes))
}

/// Compute the lowercase hex SHA-256 of `bytes`.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Path to a cached engine-API JSON file keyed by the source archive hash.
pub fn engine_cache_path(cache_dir: &Path, hash: &str) -> PathBuf {
    engine_cache_dir(cache_dir).join(format!("{hash}.json"))
}

/// Cache key for a parsed source file derived from its mtime and content hash.
pub fn parse_cache_key(mtime: SystemTime, content_hash: &str) -> String {
    let millis = mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{millis}-{content_hash}")
}

/// Path to a cached parse-tree/symbol-table JSON file.
pub fn parse_cache_path(cache_dir: &Path, key: &str) -> PathBuf {
    parse_cache_dir(cache_dir).join(format!("{key}.json"))
}

/// Serialize `value` to JSON and atomically write it to `path`.
///
/// Atomicity protects readers from partial writes if the process exits during
/// the write.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)
        .with_context(|| format!("serializing cache JSON for {path:?}"))?;

    let dir = path
        .parent()
        .with_context(|| format!("cache path has no parent: {path:?}"))?;
    fs::create_dir_all(dir)
        .with_context(|| format!("creating cache directory {dir:?}"))?;

    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("creating temporary cache file {tmp:?}"))?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("writing temporary cache file {tmp:?}"))?;
        file.sync_data().ok();
    }

    fs::rename(&tmp, path).with_context(|| {
        format!("renaming temporary cache file {tmp:?} -> {path:?}")
    })?;
    Ok(())
}

/// Deserialize the JSON file at `path` if it exists and is well-formed.
///
/// A missing file returns `Ok(None)` so callers can treat it as a cache miss.
/// A corrupt file returns an error because the caller must decide whether to
/// recover (for engine data this means re-extracting the archive).
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)
        .with_context(|| format!("reading cached JSON {path:?}"))?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(Some(value)),
        Err(e) => Err(anyhow::anyhow!(
            "corrupt cache file {path:?}: {e}. Remove the file or it will be regenerated."
        )),
    }
}

/// Load a cached engine-API value, or call `fallback` on a miss and write the
/// result back to the cache.
///
/// `archive_hash` should be the SHA-256 of the source `doxygen_retail.7z`
/// archive; callers typically compute it with [`sha256_file`].
pub fn load_or_write_engine<T, F>(
    cache_dir: &Path,
    archive_hash: &str,
    fallback: F,
) -> Result<T>
where
    T: DeserializeOwned + Serialize,
    F: FnOnce() -> Result<T>,
{
    let path = engine_cache_path(cache_dir, archive_hash);
    if let Some(value) = read_json(&path)? {
        return Ok(value);
    }

    let value = fallback().with_context(|| {
        format!(
            "extracting engine data for archive hash {archive_hash}; \
             cache file {path:?} could not be populated"
        )
    })?;

    write_json(&path, &value)
        .with_context(|| format!("writing engine cache file {path:?}"))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn cache_dirs_are_created_on_demand() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        ensure_cache_dirs(root).unwrap();
        assert!(engine_cache_dir(root).is_dir());
        assert!(parse_cache_dir(root).is_dir());
    }

    #[test]
    fn sha256_of_known_string_matches_expected() {
        // "hello" -> 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        assert_eq!(
            sha256_bytes(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn write_and_read_json_round_trip() {
        #[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
        struct Value {
            items: Vec<String>,
        }

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("v1").join("test.json");
        let value = Value {
            items: vec!["a".into(), "b".into()],
        };

        assert!(read_json::<Value>(&path).unwrap().is_none());
        write_json(&path, &value).unwrap();
        let loaded = read_json::<Value>(&path).unwrap().expect("cache hit after write");
        assert_eq!(loaded, value);
    }

    #[test]
    fn load_or_write_engine_uses_warm_cache() {
        #[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
        struct Value {
            n: i32,
        }

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let mut calls = 0usize;

        // Cold start: fallback runs once.
        let v1 = load_or_write_engine(root, "deadbeef", || {
            calls += 1;
            Ok(Value { n: 42 })
        })
        .unwrap();
        assert_eq!(v1.n, 42);
        assert_eq!(calls, 1);

        // Warm start: load from disk, fallback does not run.
        let v2 = load_or_write_engine(root, "deadbeef", || {
            calls += 1;
            Ok(Value { n: 99 })
        })
        .unwrap();
        assert_eq!(v2.n, 42);
        assert_eq!(calls, 1);
    }

    #[test]
    fn corrupt_cache_file_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("broken.json");
        fs::write(&path, b"{invalid json").unwrap();
        assert!(read_json::<serde_json::Value>(&path).is_err());
    }

    #[test]
    fn parse_cache_key_uses_mtime_and_hash() {
        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(12345);
        let key = parse_cache_key(mtime, "abc123");
        assert!(key.starts_with("12345-"));
        assert!(key.ends_with("abc123"));
    }
}
