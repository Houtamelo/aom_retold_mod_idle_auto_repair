//! Persistent cache for expensive engine-data extraction and per-file parse
//! results.
//!
//! Layout under the cache root (`~/.local/state/aomr_lsp/` or the fallback
//! `~/.aomr_lsp/`):
//!
//! ```text
//! v2/<sha256-of-doxygen_retail.7z>.json          engine API (syscalls + aiplans)
//! game_parse/v1/<mtime>-<sha256>.json            per-file parse tree + symbols
//! ```
//!
//! The `v2/` prefix for engine data is the current schema-version marker.
//! It was bumped from `v1/` when the legacy JSON backfill was removed, so
//! existing `v1/` caches are left in place and the server writes/reads only
//! `v2/` files. Future incompatible schema changes can write to `v3/` etc.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{parser, symbols};

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

/// `v2/` subdirectory for engine API cache files.
pub fn engine_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("v2")
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

/// A single entry in the per-file parse symbol cache.
///
/// The raw tree-sitter `Tree` is intentionally NOT cached — it cannot be
/// cheaply serialized across process restarts. We cache the extracted
/// `SymbolTable` and re-parse on demand to obtain the tree for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCacheEntry {
    pub relative_path: String,
    pub mtime_millis: u128,
    pub content_hash: String,
    pub symbols: symbols::SymbolTable,
}

/// Compute the cache key (`mtime-<sha256>`) and content hash for `path`.
///
/// Hashes the raw file bytes — not UTF-8-decoded text — so binary files
/// (AoM:R's `.bar` asset archives, the engine's `.dtt` data tables, etc.)
/// don't trigger a UTF-8 decode error when the cache key is computed for
/// them. SHA-256 of bytes is well-defined for both text and binary input,
/// and the hash only feeds an equality comparison in [load_or_parse_symbols],
/// so it never needs to round-trip through the filesystem as text.
pub fn parse_file_key(path: &Path) -> Result<(String, String, u128)> {
    let meta = fs::metadata(path)
        .with_context(|| format!("reading metadata for parse cache key: {path:?}"))?;
    let mtime = meta
        .modified()
        .with_context(|| format!("reading mtime for parse cache key: {path:?}"))?;
    let bytes = fs::read(path)
        .with_context(|| format!("reading file for parse cache key: {path:?}"))?;
    let hash = sha256_bytes(&bytes);
    let millis = mtime.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let key = format!("{millis}-{hash}");
    Ok((key, hash, millis))
}

/// Load the cached `SymbolTable` for a game-folder file if it matches the
/// file's current mtime and SHA-256, otherwise parse the file, extract the
/// symbol table, and write a new cache entry.
///
/// `relative_path` is the file's path relative to the game folder (e.g.
/// `ai/core/core.xs`). It is stored in the cache entry so invalidation has
/// a stable key even when the mtime/hash changes.
pub fn load_or_parse_symbols(
    path: &Path,
    relative_path: &str,
    cache_dir: &Path,
) -> Result<symbols::SymbolTable> {
    // Defensive: even though the workspace walkers filter by extension and
    // UTF-8 head probe, guard the cache layer too. If a non-`.xs` path ever
    // reaches here, return an empty table rather than attempting to
    // UTF-8-decode binary data (.bar archives, .dtt tables, .png textures,
    // binary `.xs` random-map data, etc.) and crashing every semantic check
    // that touches this file's include graph.
    if !crate::workspace::is_xs_file(path) {
        return Ok(symbols::SymbolTable::default());
    }

    // First try to use a cached parse result. If anything fails — missing
    // cache, IO error during cache read, JSON decode error — fall back to
    // a fresh parse.
    let (key, content_hash, mtime_millis) = match parse_file_key(path) {
        Ok(parts) => parts,
        Err(e) => {
            tracing::warn!(
                "could not compute cache key for {}: {}; treating as empty",
                path.display(),
                e
            );
            return Ok(symbols::SymbolTable::default());
        }
    };
    let cache_path = parse_cache_path(cache_dir, &key);

    if let Ok(Some(entry)) = read_json::<ParseCacheEntry>(&cache_path) {
        if entry.relative_path == relative_path
            && entry.content_hash == content_hash
            && entry.mtime_millis == mtime_millis
        {
            return Ok(entry.symbols);
        }
    }

    // Read the source. If it isn't valid UTF-8 (e.g. an AoM:R random-map
    // serialised `.xs` that slipped past the walker), treat the file as
    // empty for this compilation rather than aborting the whole project
    // build. The file is still in the include graph; it just contributes
    // no symbols.
    let text = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                "could not read {} as UTF-8: {}; treating as empty for this compilation",
                path.display(),
                e
            );
            return Ok(symbols::SymbolTable::default());
        }
    };
    let tree = match parser::parse(&text) {
        Some(t) => t,
        None => {
            tracing::warn!(
                "tree-sitter XS parser failed to install for {}; treating as empty for this compilation",
                path.display(),
            );
            return Ok(symbols::SymbolTable::default());
        }
    };
    let table = symbols::build_symbol_table(&tree, &text);

    // Best-effort write to cache. A failure here doesn't affect correctness.
    let entry = ParseCacheEntry {
        relative_path: relative_path.to_string(),
        mtime_millis,
        content_hash,
        symbols: table.clone(),
    };
    if let Err(e) = write_json(&cache_path, &entry) {
        tracing::debug!(
            "could not write parse cache {} (non-fatal): {}",
            cache_path.display(),
            e
        );
    }

    Ok(table)
}

/// Remove all cached parse entries whose `relative_path` matches `relative_path`.
///
/// Because the cache file name is keyed by mtime+hash rather than by path,
/// we scan the parse-cache directory and delete every matching entry. The
/// directory is small in practice.
pub fn invalidate_parse_cache(relative_path: &str, cache_dir: &Path) -> Result<()> {
    let dir = parse_cache_dir(cache_dir);
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&dir)
        .with_context(|| format!("reading parse cache directory {dir:?}"))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match read_json::<ParseCacheEntry>(&path) {
            Ok(Some(cached)) if cached.relative_path == relative_path => {
                fs::remove_file(&path)
                    .with_context(|| format!("removing stale parse cache {path:?}"))?;
            }
            _ => {}
        }
    }
    Ok(())
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

    #[test]
    fn load_or_parse_symbols_caches_and_reuses_symbol_table() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let src_dir = tmp.path().join("game");
        let file = src_dir.join("ai").join("core").join("core.xs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "void myCore() {}\n").unwrap();

        // Cold load: parse + cache.
        let t1 = load_or_parse_symbols(&file, "ai/core/core.xs", &cache_dir).unwrap();
        assert!(t1.find("myCore").is_some());
        let files: Vec<_> = fs::read_dir(parse_cache_dir(&cache_dir))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(files.len(), 1);

        // Warm load: cache hit, no new file written.
        let t2 = load_or_parse_symbols(&file, "ai/core/core.xs", &cache_dir).unwrap();
        assert!(t2.find("myCore").is_some());
        let files: Vec<_> = fs::read_dir(parse_cache_dir(&cache_dir))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn invalidate_parse_cache_removes_matching_entries() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let src_dir = tmp.path().join("game");
        let file = src_dir.join("ai").join("core").join("core.xs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "void myCore() {}\n").unwrap();

        load_or_parse_symbols(&file, "ai/core/core.xs", &cache_dir).unwrap();
        assert!(!parse_cache_dir(&cache_dir).read_dir().unwrap().flatten().next().is_none());

        invalidate_parse_cache("ai/core/core.xs", &cache_dir).unwrap();
        assert!(parse_cache_dir(&cache_dir).read_dir().unwrap().flatten().next().is_none());
    }

    #[test]
    fn load_or_parse_reparses_after_invalidation() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let src_dir = tmp.path().join("game");
        let file = src_dir.join("ai").join("core").join("core.xs");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "void oldName() {}\n").unwrap();

        let _ = load_or_parse_symbols(&file, "ai/core/core.xs", &cache_dir).unwrap();

        // Wait a millisecond so the mtime changes.
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&file, "void newName() {}\n").unwrap();

        let t = load_or_parse_symbols(&file, "ai/core/core.xs", &cache_dir).unwrap();
        assert!(t.find("newName").is_some());
        assert!(t.find("oldName").is_none());
    }

    /// Regression guard for the user's complaint:
    ///   "The LSP needs to be more robust, it can't give up on every
    ///    single bad file. If a file fails to parse, it should simply
    ///    output an error message, then temporarily exclude that file
    ///    from compilation."
    ///
    /// A `.xs` file that exists but cannot be read as UTF-8 (the AoM:R
    /// random-map binary case) MUST return `Ok(default)` from
    /// `load_or_parse_symbols` — NOT `Err`. The semantic pipeline needs
    /// the empty table to continue processing the rest of the workspace.
    #[test]
    fn load_or_parse_symbols_returns_default_on_unreadable_file() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let file = tmp.path().join("binary.xs");
        // Random-map-style binary content: invalid UTF-8 sequence at byte 1.
        fs::write(&file, [0xffu8, 0xfe, 0x00, 0xab, 0xcd, 0xef]).unwrap();

        let result = load_or_parse_symbols(&file, "ai/binary.xs", &cache_dir);
        assert!(
            result.is_ok(),
            "load_or_parse_symbols must return Ok for an unreadable .xs file; got {result:?}"
        );
        let table = result.unwrap();
        assert!(
            table.symbols.is_empty(),
            "unreadable .xs file should produce an empty SymbolTable, got {} symbols",
            table.symbols.len()
        );
    }

    /// A `.xs` file that doesn't exist on disk (e.g. stale include target)
    /// MUST also return `Ok(default)` rather than `Err`. The semantic
    /// pipeline treats this as "no symbols contributed by this file" and
    /// moves on.
    #[test]
    fn load_or_parse_symbols_returns_default_on_missing_file() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let file = tmp.path().join("does_not_exist.xs");

        let result = load_or_parse_symbols(&file, "ai/does_not_exist.xs", &cache_dir);
        assert!(
            result.is_ok(),
            "load_or_parse_symbols must return Ok for a missing .xs file; got {result:?}"
        );
        let table = result.unwrap();
        assert!(table.symbols.is_empty());
    }

    /// A non-`.xs` file (e.g. a `.bar` archive that somehow reaches the
    /// cache layer through a regression in the upstream filters) MUST
    /// return `Ok(default)` immediately without attempting to parse it.
    /// Without this guard, the cache layer would crash with a UTF-8
    /// decode error the first time the walker ever missed a filter.
    #[test]
    fn load_or_parse_symbols_returns_default_on_non_xs_extension() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("cache");
        let file = tmp.path().join("asset.bar");
        fs::write(&file, [0xffu8, 0xfe, 0x00]).unwrap();

        let result = load_or_parse_symbols(&file, "asset.bar", &cache_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().symbols.is_empty());
    }
}
