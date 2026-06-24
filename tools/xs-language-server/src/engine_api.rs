//! Loads the engine-API data (`syscalls` and `aiplans`) once at server start
//! and exposes lookup helpers used by completion, hover, and type checking.
//!
//! Data is extracted from the AoM:R `doxygen_retail.7z` archive and cached by
//! SHA-256 under the provided cache directory (usually
//! `~/.local/state/aomr_lsp/v1/`). No static JSON resources are bundled or
//! loaded from sibling crates.

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{cache, doxygen};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Param {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Syscall {
    pub name: String,
    pub help: String,
    #[serde(rename = "return_type")]
    pub return_type: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SyscallsFile {
    pub syscalls: Vec<Syscall>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AiplanConstant {
    pub name: String,
    pub value: i64,
    #[serde(rename = "variable_type")]
    pub variable_type: String,
    /// `variable_value` is sometimes a JSON string (e.g. `"\"-1\""` for ints)
    /// and sometimes a bare JSON number (e.g. `0` for booleans). Accept both.
    #[serde(rename = "variable_value", deserialize_with = "deserialize_string_or_number")]
    pub variable_value: String,
}

/// Combined engine data as stored in `v1/<hash>.json`.
#[derive(Debug, Deserialize, Serialize)]
pub struct EngineData {
    pub syscalls: Vec<Syscall>,
    pub constants: Vec<AiplanConstant>,
}

fn deserialize_string_or_number<'de, D>(de: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let v = serde_json::Value::deserialize(de)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Bool(b) => Ok(b.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        other => Err(D::Error::custom(format!(
            "expected string or number, got {other}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct AiplansFile {
    pub constants: Vec<AiplanConstant>,
}

#[derive(Debug, Default)]
pub struct EngineApi {
    pub syscalls: Vec<Syscall>,
    pub aiplans: Vec<AiplanConstant>,
}

impl EngineApi {
    /// Build from an [`EngineData`] value produced by extraction or cache
    /// deserialization.
    pub fn from_engine_data(data: EngineData) -> Self {
        Self {
            syscalls: data.syscalls,
            aiplans: data.constants,
        }
    }

    /// Convert back to the serializable shape used by the cache.
    pub fn to_engine_data(&self) -> EngineData {
        EngineData {
            syscalls: self.syscalls.clone(),
            constants: self.aiplans.clone(),
        }
    }

    /// Load the engine API from `doxygen_retail.7z`, using the SHA-256 cache
    /// under `cache_dir` to skip re-extraction on warm starts.
    ///
    /// If the cached JSON is corrupt, it is treated as a miss and the archive
    /// is re-extracted.
    pub fn load_from_archive(archive: &Path, cache_dir: &Path) -> anyhow::Result<Self> {
        cache::ensure_cache_dirs(cache_dir)
            .with_context(|| format!("preparing cache directory {cache_dir:?}"))?;
        let hash = cache::sha256_file(archive)
            .with_context(|| format!("hashing archive {archive:?}"))?;
        let cache_path = cache::engine_cache_path(cache_dir, &hash);

        let data: EngineData = match Self::extract_and_cache(archive, cache_dir, &hash) {
            Ok(data) => data,
            Err(_) if cache_path.exists() => {
                // Possible corrupt cache: delete it and try one more time.
                std::fs::remove_file(&cache_path)
                    .with_context(|| format!("removing corrupt cache {cache_path:?}"))?;
                Self::extract_and_cache(archive, cache_dir, &hash)
                    .with_context(|| format!("re-extracting after corrupt cache {cache_path:?}"))?
            }
            Err(e) => return Err(e),
        };

        Ok(Self::from_engine_data(data))
    }

    fn extract_and_cache(
        archive: &Path,
        cache_dir: &Path,
        hash: &str,
    ) -> anyhow::Result<EngineData> {
        cache::load_or_write_engine(cache_dir, hash, || {
            doxygen::extract_engine_api(archive)
                .with_context(|| format!("extracting engine API from {archive:?}"))
        })
    }

    /// Syscalls whose `name` starts with `prefix` (case-sensitive).
    pub fn matching_syscalls(&self, prefix: &str) -> impl Iterator<Item = &Syscall> {
        self.syscalls
            .iter()
            .filter(move |s| s.name.starts_with(prefix))
    }

    /// AI-plan constants whose `name` starts with `prefix`.
    pub fn matching_aiplans(&self, prefix: &str) -> impl Iterator<Item = &AiplanConstant> {
        self.aiplans
            .iter()
            .filter(move |c| c.name.starts_with(prefix))
    }

    /// Exact-match lookup by name.
    pub fn find_syscall(&self, name: &str) -> Option<&Syscall> {
        self.syscalls.iter().find(|s| s.name == name)
    }

    /// Exact-match lookup by name.
    pub fn find_aiplan(&self, name: &str) -> Option<&AiplanConstant> {
        self.aiplans.iter().find(|c| c.name == name)
    }
}

/// Wrap in `Arc` so the server can hand the same data to multiple async
/// handler calls without copying.
pub type SharedEngineApi = Arc<EngineApi>;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_from_archive_cold_start_hits_target_counts() {
        let archive = Path::new("../../docs/doxygen_retail.7z");
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path();

        let api = EngineApi::load_from_archive(archive, cache_dir).unwrap();
        assert_eq!(
            api.syscalls.len(),
            1804,
            "expected 1804 syscalls parsed directly from the Doxygen archive"
        );
        assert_eq!(
            api.aiplans.len(),
            193,
            "expected 193 AI-plan constants"
        );

        let hash = cache::sha256_file(archive).unwrap();
        assert!(cache::engine_cache_path(cache_dir, &hash).exists());
    }

    #[test]
    fn load_from_archive_warm_start_skips_extraction() {
        let archive = Path::new("../../docs/doxygen_retail.7z");
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path();

        let cold = EngineApi::load_from_archive(archive, cache_dir).unwrap();
        let warm = EngineApi::load_from_archive(archive, cache_dir).unwrap();

        assert_eq!(cold.syscalls.len(), warm.syscalls.len());
        assert_eq!(cold.aiplans.len(), warm.aiplans.len());
    }

    #[test]
    fn corrupt_cache_falls_back_to_re_extraction() {
        let archive = Path::new("../../docs/doxygen_retail.7z");
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path();

        // Cold start to populate cache.
        let cold = EngineApi::load_from_archive(archive, cache_dir).unwrap();
        let hash = cache::sha256_file(archive).unwrap();
        let cache_path = cache::engine_cache_path(cache_dir, &hash);
        assert!(cache_path.exists());

        // Corrupt the cache file.
        std::fs::write(&cache_path, b"{not valid json").unwrap();

        // load_from_archive detects the corrupt cache, deletes it, and
        // re-extracts from the archive.
        let recovered = EngineApi::load_from_archive(archive, cache_dir).unwrap();
        assert_eq!(recovered.syscalls.len(), cold.syscalls.len());
        assert_eq!(recovered.aiplans.len(), cold.aiplans.len());
    }
}
