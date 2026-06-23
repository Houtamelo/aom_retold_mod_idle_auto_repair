//! Loads the engine-API data (`syscalls.json` and `aiplans.json`) once at
//! server start and exposes lookup helpers used by the completion handler.
//!
//! Source files live under `tools/intellij-xs-plugin/src/main/resources/`;
//! this crate reaches them by relative path. Long-term the Rust crate
//! becomes the source of truth (see `docs/xs-lsp-spike.md`, "What gets
//! replaced").

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

const DEFAULT_DATA_DIR: &str =
    "../intellij-xs-plugin/src/main/resources";

#[derive(Debug, Deserialize, Clone)]
pub struct Param {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Syscall {
    pub name: String,
    pub help: String,
    #[serde(rename = "return_type")]
    pub return_type: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SyscallsFile {
    pub syscalls: Vec<Syscall>,
}

#[derive(Debug, Deserialize, Clone)]
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
    /// Try to load from the standard sibling-crate path. Returns `None`
    /// (not an error) if the files aren't present, so the server can still
    /// start in fresh checkouts where the IntelliJ plugin hasn't been
    /// built yet.
    pub fn load_default() -> Option<Self> {
        Self::load_from_dir(Path::new(DEFAULT_DATA_DIR))
    }

    pub fn load_from_dir(dir: &Path) -> Option<Self> {
        let syscalls = fs::read_to_string(dir.join("syscalls.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<SyscallsFile>(&s).ok())
            .map(|f| f.syscalls)
            .unwrap_or_default();
        let aiplans = fs::read_to_string(dir.join("aiplans.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<AiplansFile>(&s).ok())
            .map(|f| f.constants)
            .unwrap_or_default();
        if syscalls.is_empty() && aiplans.is_empty() {
            return None;
        }
        Some(Self { syscalls, aiplans })
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