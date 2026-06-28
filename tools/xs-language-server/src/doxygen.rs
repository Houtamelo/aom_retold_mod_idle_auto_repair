//! Extract the AoM:R engine API from the Doxygen archive shipped with the game.
//!
//! The archive contains HTML generated from the developer C++ sources. This
//! module decompresses the archive to a temp dir, scrapes the relevant
//! `*_8cpp.html` pages, and produces the `EngineData` JSON shape consumed by
//! [`crate::engine_api`].

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};
use tracing::warn;

use crate::engine_api::{AiplanConstant, EngineData, Param, Syscall};

static SELECTOR_MEMITEM: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.memitem").expect("valid selector"));
static SELECTOR_MEMPROTO: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.memproto").expect("valid selector"));
static SELECTOR_MEMNAME: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td.memname").expect("valid selector"));
static SELECTOR_PARAMS_DL_TR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("dl.params tr").expect("valid selector"));
static SELECTOR_SUMMARY_ITEM: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"tr[class^="memitem"]"#).expect("valid selector"));
static SELECTOR_SUMMARY_LEFT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td.memItemLeft").expect("valid selector"));
static SELECTOR_SUMMARY_RIGHT: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("td.memItemRight").expect("valid selector"));
static SELECTOR_SUMMARY_DESC: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(r#"tr[class^="memdesc"]"#).expect("valid selector"));


/// Extract syscalls and AI-plan constants from `<game-path>/doxygen_retail.7z`.
///
/// Extraction is performed into a temporary directory that is deleted when the
/// returned value is dropped.
pub fn extract_engine_api(archive: &Path) -> Result<EngineData> {
    let tmp = tempfile::tempdir().with_context(|| format!("creating temp dir for {archive:?}"))?;
    sevenz_rust::decompress_file(archive, tmp.path())
        .with_context(|| format!("decompressing 7z archive {archive:?}"))?;

    let mut function_files: Vec<(String, PathBuf)> = Vec::new();
    let mut aiplans_file: Option<PathBuf> = None;

    collect_html_files(tmp.path(), &mut function_files, &mut aiplans_file)?;

    let mut syscalls = Vec::new();
    for (filename, path) in function_files {
        let mut parsed = parse_function_file(&path, &filename)?;
        syscalls.append(&mut parsed);
    }

    let constants = if let Some(path) = aiplans_file {
        parse_aiplans_file(&path)?
    } else {
        warn!("doxygen archive is missing aiplans_8cpp.html; engine data will have no plan constants");
        Vec::new()
    };

    if syscalls.is_empty() {
        anyhow::bail!(
            "no engine syscalls were extracted from {archive:?}; \
             the archive may be corrupt or use an unsupported Doxygen layout"
        );
    }

    if constants.is_empty() {
        warn!(
            "no AI-plan constants were extracted from {archive:?}; \
             continuing with syscalls only"
        );
    }

    Ok(EngineData {
        syscalls,
        constants,
    })
}

/// Recursively walk `dir` and collect all `*_8cpp.html` Doxygen pages.
fn collect_html_files(
    dir: &Path,
    function_files: &mut Vec<(String, PathBuf)>,
    aiplans_file: &mut Option<PathBuf>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("reading {dir:?}"))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy();

        if path.is_dir() {
            collect_html_files(&path, function_files, aiplans_file)?;
        } else if name_lossy.ends_with("_8cpp.html") {
            if name_lossy == "aiplans_8cpp.html" {
                *aiplans_file = Some(path);
            } else {
                let cpp_name = html_filename_to_cpp(&name_lossy);
                function_files.push((cpp_name, path));
            }
        }
    }

    Ok(())
}

/// `aifuncs_8cpp.html` -> `aifuncs.cpp`, `triggerfuncs__mainthread_8cpp.html`
/// -> `triggerfuncs_mainthread.cpp`.
fn html_filename_to_cpp(html_name: &str) -> String {
    let base = html_name
        .strip_suffix("_8cpp.html")
        .unwrap_or(html_name);
    let base = base.replace("__", "_");
    format!("{base}.cpp")
}

/// Parse one function-reference page into syscalls.
fn parse_function_file(path: &Path, source_filename: &str) -> Result<Vec<Syscall>> {
    let html = fs::read_to_string(path)
        .with_context(|| format!("reading {path:?}"))?;
    let doc = Html::parse_document(&html);

    // The summary table at the top of the page lists every function, including
    // ones without a detailed documentation block. The detailed blocks provide
    // parameter defaults, so index those by function name.
    let defaults_by_name = parse_detail_defaults(&doc);
    let help_by_hash = parse_summary_descriptions(&doc);

    let mut syscalls = Vec::new();
    for row in doc.select(&SELECTOR_SUMMARY_ITEM) {
        let Some(class) = row.value().attr("class") else {
            continue;
        };
        let Some(hash) = class.strip_prefix("memitem:") else {
            continue;
        };

        let Some(return_type) = row
            .select(&SELECTOR_SUMMARY_LEFT)
            .next()
            .map(|e| normalize_text(&e.text().collect::<String>()))
        else {
            warn!("function summary row has no return type in {path:?}");
            continue;
        };
        let Some(right_text) = row
            .select(&SELECTOR_SUMMARY_RIGHT)
            .next()
            .map(|e| normalize_text(&e.text().collect::<String>()))
        else {
            warn!("function summary row has no signature in {path:?}");
            continue;
        };

        let Some((name, params_text)) = parse_summary_signature(&right_text) else {
            warn!("skipping malformed summary signature {right_text:?} in {path:?}");
            continue;
        };

        let help = help_by_hash
            .get(hash)
            .cloned()
            .unwrap_or_default();
        let defaults = defaults_by_name
            .get(&name)
            .cloned()
            .unwrap_or_default();

        let params: Vec<Param> = parse_params_text(&params_text)
            .into_iter()
            .map(|(ty, pname)| Param {
                ty,
                name: pname.clone(),
                default: defaults.get(&pname).cloned(),
                is_ref: false,
            })
            .collect();

        syscalls.push(Syscall {
            name,
            help,
            return_type,
            params,
            filename: Some(source_filename.to_string()),
        });
    }

    Ok(syscalls)
}

/// Build a map from summary-row hash to the one-line description in the
/// adjacent `memdesc` row.
fn parse_summary_descriptions(doc: &Html) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for row in doc.select(&SELECTOR_SUMMARY_DESC) {
        let Some(class) = row.value().attr("class") else { continue; };
        let Some(hash) = class.strip_prefix("memdesc:") else { continue; };
        let text = normalize_text(&row.text().collect::<String>());
        map.insert(hash.to_string(), text);
    }
    map
}

/// Parse `aiAddEchoCategory (string categoryName)` from a summary cell.
fn parse_summary_signature(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let Some(open) = text.find('(') else { return None; };
    let Some(close) = text.rfind(')') else { return None; };
    let name = text[..open].trim().to_string();
    let params = text[open + 1..close].to_string();
    if name.is_empty() || !name.chars().next()?.is_alphabetic() {
        return None;
    }
    Some((name, params))
}

/// Parse a comma-separated parameter list into `(type, name)` pairs.
fn parse_params_text(params_text: &str) -> Vec<(String, String)> {
    if params_text.trim().is_empty() {
        return Vec::new();
    }

    params_text
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter_map(|param| {
            let tokens: Vec<&str> = param.split_whitespace().collect();
            if tokens.len() < 2 {
                return None;
            }
            let name = tokens.last()?.to_string();
            let ty = tokens[..tokens.len() - 1].join(" ");
            Some((ty, name))
        })
        .collect()
}

/// Index detailed documentation blocks by function name so defaults can be
/// attached to the summary-row results.
fn parse_detail_defaults(doc: &Html) -> HashMap<String, HashMap<String, String>> {
    let mut map = HashMap::new();
    for item in doc.select(&SELECTOR_MEMITEM) {
        if item.select(&SELECTOR_MEMPROTO).next().is_none() {
            continue;
        }
        let Some((_, name)) = parse_memproto_signature(&item) else {
            continue;
        };
        map.insert(name, parse_memdoc_defaults(&item));
    }
    map
}

/// Parse `int aiAddEchoCategory` from the first `<td class="memname">` cell.
fn parse_memproto_signature(item: &ElementRef<'_>) -> Option<(String, String)> {
    let td = item.select(&SELECTOR_MEMNAME).next()?;
    let text = normalize_text(&td.text().collect::<String>());

    // Examples:
    //   "int aiAddEchoCategory"
    //   "void aiAddToResourceBreakdown"
    //   "int[] aiPlanGetIDsByType"
    // A few entries have additional qualification such as "static"; drop it.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }
    let name = tokens.last()?.to_string();
    if !name.chars().next()?.is_alphabetic() {
        return None;
    }
    // Return type is everything before the name.
    let return_type = tokens[..tokens.len() - 1].join(" ");
    Some((return_type, name))
}

/// Map parameter name -> default value from `<dl class="params">` rows.
fn parse_memdoc_defaults(item: &ElementRef<'_>) -> HashMap<String, String> {
    let mut defaults = HashMap::new();

    for row in item.select(&SELECTOR_PARAMS_DL_TR) {
        let cells: Vec<String> = row
            .select(&Selector::parse("td").expect("valid selector"))
            .map(|e| normalize_text(&e.text().collect::<String>()))
            .collect();
        if cells.len() < 2 {
            continue;
        }
        let pname = &cells[0];
        let desc = &cells[1];
        if let Some(value) = desc.strip_prefix("default value:") {
            defaults.insert(pname.clone(), value.trim().to_string());
        }
    }

    defaults
}

/// Parse AI-plan constants from `aiplans_8cpp.html`.
fn parse_aiplans_file(path: &Path) -> Result<Vec<AiplanConstant>> {
    let html = fs::read_to_string(path)
        .with_context(|| format!("reading {path:?}"))?;
    let doc = Html::parse_document(&html);

    let desc_by_hash = parse_summary_descriptions(&doc);
    let mut constants = Vec::new();

    for row in doc.select(&SELECTOR_SUMMARY_ITEM) {
        let Some(class) = row.value().attr("class") else { continue; };
        let Some(hash) = class.strip_prefix("memitem:") else { continue; };

        let Some(right_text) = row
            .select(&SELECTOR_SUMMARY_RIGHT)
            .next()
            .map(|e| normalize_text(&e.text().collect::<String>()))
        else {
            continue;
        };

        let Some((name, value_str)) = parse_aiplan_name_value(&right_text) else {
            continue;
        };
        let Ok(value) = value_str.parse::<i64>() else {
            warn!("skipping AI-plan constant with non-integer value: {right_text:?}");
            continue;
        };

        let desc = desc_by_hash
            .get(hash)
            .cloned()
            .unwrap_or_default();
        let (variable_type, variable_value) = parse_aiplan_description(&desc);

        constants.push(AiplanConstant {
            name,
            value,
            variable_type,
            variable_value,
        });
    }

    Ok(constants)
}

/// Parse `cAttackPlanAttackRouteID = 0` from a summary cell.
fn parse_aiplan_name_value(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let Some(eq) = text.find('=') else { return None; };
    let name = text[..eq].trim().to_string();
    let value = text[eq + 1..].trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some((name, value))
}

/// Convert `variable type = int, initial value = -1` into `(type, value)`.
fn parse_aiplan_description(description: &str) -> (String, String) {
    let mut variable_type = "?".to_string();
    let mut variable_value = String::new();

    for token in description.split(',') {
        let token = token.trim();
        if let Some(ty) = token.strip_prefix("variable type =") {
            variable_type = ty.trim().to_string();
        } else if let Some(v) = token.strip_prefix("initial value =") {
            variable_value = v.trim().to_string();
        }
    }

    (variable_type, variable_value)
}

/// Collapse whitespace and trim HTML-derived text.
fn normalize_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_docs_doxygen_retail_counts() {
        let archive = Path::new("../../docs/doxygen_retail.7z");
        let data = extract_engine_api(archive).expect("extract docs/doxygen_retail.7z");
        // The archive contains 1804 function summary rows. `xsExecute` is not
        // present in the archive, so the server exposes exactly what Doxygen
        // provides (no legacy JSON backfill).
        assert_eq!(
            data.syscalls.len(), 1804,
            "expected 1804 syscalls parsed directly from docs/doxygen_retail.7z"
        );
        assert_eq!(
            data.constants.len(), 193,
            "expected 193 AI-plan constants from docs/doxygen_retail.7z"
        );
    }
}
