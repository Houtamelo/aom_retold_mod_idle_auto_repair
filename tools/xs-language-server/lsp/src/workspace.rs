//! Virtual-project workspace model.
//!
//! Each registered Age of Mythology: Retold mod is represented as a
//! `VirtualProject` composed of:
//!
//!   1. the extracted engine API (always present),
//!   2. the vanilla `game/` folder under the user's game path, and
//!   3. the mod's `game/` overlay, which hides vanilla files at the same
//!      relative path.
//!
//! `include "..."` resolution uses include-root context inferred from the
//! file's location (`game/ai/`, `game/data/trigger/`, `game/random_maps/`).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tower_lsp_server::ls_types::Uri;

/// A registered mod workspace folder.
#[derive(Debug, Clone)]
pub struct ModEntry {
    /// The LSP workspace-folder URI for this mod root.
    pub mod_uri: Uri,
    /// Absolute filesystem path of the mod root (parent of `game/`).
    pub mod_path: PathBuf,
    /// Absolute filesystem path of the mod overlay: `mod_path.join("game")`.
    pub overlay_path: PathBuf,
}

/// A mod's resolved view of the game folder.
#[derive(Debug, Clone, Default)]
pub struct VirtualProject {
    /// Relative paths under `game/` (e.g. `ai/human_assist/human_assist.xs`)
    /// mapped to the absolute mod-overlay path that replaces them.
    pub file_overrides: HashMap<String, PathBuf>,
}

impl VirtualProject {
    /// Return every file visible in this project. Files in the mod overlay
    /// hide vanilla files with the same relative path.
    ///
    /// This is a convenience for workspace-symbol style queries; it is not
    /// used for per-keystroke diagnostics.
    pub fn visible_files(&self, workspace: &Workspace) -> Vec<(String, PathBuf)> {
        let game_root = workspace.game_path.join("game");
        let game_files = walk_game_files(&game_root, &self.file_overrides);
        let mut files: Vec<(String, PathBuf)> = self.file_overrides.clone().into_iter().collect();
        files.extend(game_files);
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files.dedup_by(|a, b| a.0 == b.0);
        files
    }
}

/// The XS runtime context that determines how `include "..."` is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncludeRoot {
    /// `game/ai/...`
    Ai,
    /// `game/data/trigger/...`
    Trigger,
    /// `game/random_maps/...`
    RandomMap,
}

impl IncludeRoot {
    /// The include-root prefix relative to `game/`.
    pub fn rel_prefix(&self) -> &'static str {
        match self {
            IncludeRoot::Ai => "ai/",
            IncludeRoot::Trigger => "data/trigger/",
            IncludeRoot::RandomMap => "random_maps/",
        }
    }
}

/// Errors that can occur while registering or analysing a workspace folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// The workspace-folder URI was not a valid `file://` URI.
    NotFileUri,
    /// The mod root does not contain a `game/` directory.
    MissingGameDirectory(PathBuf),
}

/// Errors that can occur while resolving an `include "..."` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The includer's path does not lie under a known include root.
    UnknownIncludeRoot,
    /// The resolved relative path does not exist in the project or vanilla game.
    NotFound {
        /// Raw include target as it appeared in the directive.
        target: String,
        /// Include root that was used for resolution.
        root:   IncludeRoot,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::UnknownIncludeRoot => {
                write!(f, "include target has no known include root")
            }
            ResolveError::NotFound { target, root } => {
                write!(f, "include target {:?} not found under {:?} root", target, root.rel_prefix())
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// A single resolved include relationship between two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEdge {
    /// Absolute path of the file containing the `include` directive.
    pub from: PathBuf,
    /// Absolute path of the resolved include target.
    pub to: PathBuf,
    /// Include-root context used for resolution (`AI`, `TRIGGER`, `RANDOM_MAP`).
    pub root: IncludeRoot,
    /// 0-indexed line of the `include` directive in `from`.
    pub include_line: u32,
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceError::NotFileUri => write!(f, "workspace folder URI is not a file URI"),
            WorkspaceError::MissingGameDirectory(p) => {
                write!(f, "mod root does not contain a game/ directory: {}", p.display())
            }
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// Owns the set of registered mods and resolves per-file ownership.
#[derive(Debug, Clone)]
pub struct Workspace {
    game_path: PathBuf,
    mods: Vec<ModEntry>,
}

impl Workspace {
    /// Create an empty workspace rooted at `game_path`.
    pub fn new(game_path: PathBuf) -> Self {
        Self {
            game_path,
            mods: Vec::new(),
        }
    }

    /// Register a mod workspace folder.
    ///
    /// The URI must be a `file://` URI. The mod's overlay directory is
    /// expected to be `<uri>/game/`; if it does not exist the registration
    /// still succeeds (the overlay is simply empty until the directory is
    /// created).
    pub fn register_mod(&mut self, mod_uri: Uri) -> Result<(), WorkspaceError> {
        // `ls_types::Uri::to_file_path` returns `Some` for *any* URI with a
        // path component, even non-`file://` schemes, so we must check the
        // scheme explicitly. RFC 3986 schemes are case-insensitive.
        if !mod_uri.scheme().as_str().eq_ignore_ascii_case("file") {
            return Err(WorkspaceError::NotFileUri);
        }
        let mod_path = mod_uri
            .to_file_path()
            .ok_or_else(|| WorkspaceError::NotFileUri)?
            .to_path_buf();

        let overlay_path = mod_path.join("game");

        // Remove any prior entry for the same URI to avoid duplicates.
        self.unregister_mod(&mod_uri);

        self.mods.push(ModEntry {
            mod_uri,
            mod_path,
            overlay_path,
        });
        Ok(())
    }

    /// Remove a previously registered mod workspace folder.
    pub fn unregister_mod(&mut self, mod_uri: &Uri) { self.mods.retain(|m| m.mod_uri.as_str() != mod_uri.as_str()); }

    /// All currently registered mods.
    pub fn mods(&self) -> &[ModEntry] { &self.mods }

    /// Return the enclosing mod entry for `file_uri`, using the longest
    /// prefix match. Returns `None` for files outside every registered mod.
    pub fn lookup_mod(&self, file_uri: &Uri) -> Option<&ModEntry> {
        let path = file_uri.to_file_path()?;
        self.mods
            .iter()
            .filter(|m| path.starts_with(&m.mod_path))
            .max_by_key(|m| m.mod_path.as_os_str().len())
    }

    /// Build the `VirtualProject` for a registered mod.
    pub fn build_virtual_project(&self, mod_entry: &ModEntry) -> VirtualProject {
        let mut file_overrides = HashMap::new();
        if mod_entry.overlay_path.is_dir() {
            collect_overlay_files(&mod_entry.overlay_path, &mod_entry.overlay_path, &mut file_overrides);
        }
        VirtualProject { file_overrides }
    }

    /// Resolve a relative path under `game/` inside a virtual project.
    ///
    /// Mod overlays take precedence over the vanilla game folder. Resolves
    /// only paths whose file extension is `.xs` — the LSP should never treat
    /// binary `.bar` archives (AoM:R art/UI/audio assets) or `.dtt` data
    /// tables as XS source even if they live at a path the include walker
    /// tried to reach.
    pub fn resolve_file(&self, project: &VirtualProject, rel: &str) -> Option<PathBuf> {
        let path = if let Some(p) = project.file_overrides.get(rel) {
            p.clone()
        } else {
            let candidate = self.game_path.join("game").join(rel);
            if !candidate.exists() {
                return None;
            }
            candidate
        };
        if !is_xs_file(&path) {
            return None;
        }
        Some(path)
    }

    /// Resolve `include "target"` from `from_rel` inside `project`.
    ///
    /// The resolution uses the include-root context of `from_rel`, so an
    /// `include "core/core.xs"` from `game/ai/human_assist/human_assist.xs`
    /// first looks for `game/ai/core/core.xs` in the mod overlay, then in the
    /// vanilla game folder.
    pub fn resolve_include(&self, project: &VirtualProject, from_rel: &str, include_target: &str) -> Option<PathBuf> {
        let root = detect_include_root(from_rel)?;
        let rel = format!("{}{}", root.rel_prefix(), normalize_include_target(include_target));
        self.resolve_file(project, &rel)
    }

    /// Resolve an include and return both the target path and an `IncludeEdge`
    /// describing the relationship.
    pub fn resolve_include_edge(
        &self,
        project: &VirtualProject,
        from_rel: &str,
        include_target: &str,
        from_path: &Path,
        include_line: u32,
    ) -> Result<(PathBuf, IncludeEdge), ResolveError> {
        let root = detect_include_root(from_rel).ok_or(ResolveError::UnknownIncludeRoot)?;
        let target = normalize_include_target(include_target);
        let rel = format!("{}{}", root.rel_prefix(), target);
        let to_path = self.resolve_file(project, &rel).ok_or_else(|| ResolveError::NotFound {
            target: include_target.to_string(),
            root,
        })?;
        let edge = IncludeEdge {
            from: from_path.to_path_buf(),
            to: to_path.clone(),
            root,
            include_line,
        };
        Ok((to_path, edge))
    }

    /// Absolute path to a file under the game folder given its relative path
    /// under `game/`.
    pub fn game_file_path(&self, rel: &str) -> PathBuf { self.game_path.join("game").join(rel) }

    /// Relative path under `game/` for an absolute file path, if it lies inside
    /// the workspace game folder.
    pub fn game_relative_path(&self, abs_path: &Path) -> Option<String> {
        let game_root = self.game_path.join("game");
        relativize(&game_root, abs_path)
    }

    /// The game-path root used by this workspace.
    pub fn game_path(&self) -> &Path { &self.game_path }

    /// Resolve `include_target` from an absolute source file path.
    ///
    /// Computes the file's relative path under the mod overlay (if it is an
    /// override) or the vanilla `game/` folder, then delegates to
    /// [`Self::resolve_include`]. Returns `None` if the file is outside both
    /// contexts or the target cannot be resolved.
    pub fn resolve_include_for_file(
        &self,
        project: &VirtualProject,
        from_file: &Path,
        include_target: &str,
    ) -> Option<PathBuf> {
        let from_rel = self.relative_path_for_file(project, from_file);
        self.resolve_include(project, &from_rel, include_target)
    }

    /// Reverse-map an absolute file path to the relative path used for
    /// include-root detection. Mirrors `merged_view::relative_path_for`.
    fn relative_path_for_file(&self, project: &VirtualProject, abs_path: &Path) -> String {
        project
            .file_overrides
            .iter()
            .find(|(_, p)| *p == abs_path)
            .map(|(rel, _)| rel.clone())
            .or_else(|| self.game_relative_path(abs_path))
            .unwrap_or_else(|| abs_path.to_string_lossy().into_owned())
    }
}

/// Normalize an include target so backslashes from Windows-style paths are
/// treated as path separators on all platforms.
fn normalize_include_target(target: &str) -> String { target.replace('\\', "/") }

/// Infer the include-root context for a file from its path under `game/`.
pub fn detect_include_root(rel_path: &str) -> Option<IncludeRoot> {
    const ROOTS: [IncludeRoot; 3] = [IncludeRoot::Ai, IncludeRoot::Trigger, IncludeRoot::RandomMap];
    ROOTS.iter().find(|r| rel_path.starts_with(r.rel_prefix())).copied()
}

fn collect_overlay_files(overlay_root: &Path, current: &Path, out: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Do not recurse into nested `game/` directories: once we are
            // inside a mod's `game/` overlay, a deeper `game/` directory is
            // a mod-in-mod boundary and should not be treated as part of this
            // mod's overlay.
            if path.file_name().map(|n| n == "game").unwrap_or(false) {
                continue;
            }
            collect_overlay_files(overlay_root, &path, out);
        } else if path.is_file() {
            if !is_readable_xs_file(&path) {
                continue;
            }
            let Some(rel) = relativize(overlay_root, &path) else {
                continue;
            };
            out.insert(rel, path);
        }
    }
}

fn walk_game_files(game_root: &Path, overrides: &HashMap<String, PathBuf>) -> Vec<(String, PathBuf)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>, overrides: &HashMap<String, PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out, overrides);
            } else if path.is_file() {
                if !is_readable_xs_file(&path) {
                    continue;
                }
                if let Some(rel) = relativize(base, &path) {
                    if !overrides.contains_key(&rel) {
                        out.push((rel, path));
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    walk(game_root, game_root, &mut out, overrides);
    out
}

/// True if `path` is a file the LSP should treat as XS source.
///
/// The shipped AoM:R game folder and user mods contain many non-XS files
/// (binary `.bar` asset archives for art/UI/audio, `.dtt` data tables,
/// `.xml` strings, `.png`/`.dds` textures, etc.). Including those in
/// workspace walks caused the LSP to attempt to hash them as UTF-8 text
/// and crash with a `stream did not contain valid UTF-8` error the moment
/// any semantic check ran against a real mod file. Filter at every entry
/// point so the LSP never reads a non-`.xs` file as source.
pub fn is_xs_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xs"))
        .unwrap_or(false)
}

/// True if `path` is an `.xs` file AND its first 64 KiB decode as valid
/// UTF-8. AoM:R ships 20 files under `game/random_maps/` with a `.xs`
/// extension that are actually binary serialised data (custom AoM:R
/// format, not XS source); the engine parses them differently and the
/// LSP must skip them too, not just non-`.xs` artefacts.
///
/// We only probe the head because:
///   1. A valid UTF-8 BOM / leading comment is always present in real
///      XS files, so 64 KiB is more than enough to discriminate.
///   2. Random-map files start with binary bytes within the first
///      hundred bytes, so a tiny probe catches them cheaply.
///   3. Avoiding a full read keeps the workspace walk cheap across
///      the game folder's hundreds of files.
pub fn is_readable_xs_file(path: &Path) -> bool {
    if !is_xs_file(path) {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    use std::io::Read;
    let mut buf = [0u8; 65536];
    let Ok(n) = f.read(&mut buf) else {
        return false;
    };
    std::str::from_utf8(&buf[..n]).is_ok()
}

/// Compute the POSIX-style relative path of `file` with respect to `base`,
/// using `/` as the separator regardless of the host platform.
pub(crate) fn relativize(base: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(base).ok()?;
    let rel_str = rel.to_string_lossy();
    let normalized = if std::path::MAIN_SEPARATOR == '/' {
        rel_str.into_owned()
    } else {
        rel_str.replace(std::path::MAIN_SEPARATOR, "/")
    };
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use std::{io::Write, str::FromStr};

    use tempfile::TempDir;

    use super::*;
    use crate::parser;

    fn touch(path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut f = std::fs::File::create(path)?;
        f.write_all(b"// xs\n")?;
        Ok(())
    }

    fn write(path: &Path, content: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, content)
    }

    #[test]
    fn detect_include_root_recognises_ai() {
        assert_eq!(detect_include_root("ai/human_assist/foo.xs"), Some(IncludeRoot::Ai));
        assert_eq!(detect_include_root("ai/core/core.xs"), Some(IncludeRoot::Ai));
    }

    #[test]
    fn detect_include_root_recognises_trigger() {
        assert_eq!(detect_include_root("data/trigger/foo.xs"), Some(IncludeRoot::Trigger));
    }

    #[test]
    fn detect_include_root_recognises_random_map() {
        assert_eq!(detect_include_root("random_maps/foo.xs"), Some(IncludeRoot::RandomMap));
    }

    #[test]
    fn detect_include_root_returns_none_for_unknown() {
        assert_eq!(detect_include_root("ui/foo.xs"), None);
    }

    #[test]
    fn register_mod_requires_file_uri() {
        let tmp = TempDir::new().unwrap();
        let mut ws = Workspace::new(tmp.path().to_path_buf());
        let uri = Uri::from_str("http://example.com/mod").unwrap();
        assert_eq!(ws.register_mod(uri).unwrap_err(), WorkspaceError::NotFileUri);
    }

    #[test]
    fn lookup_mod_uses_longest_prefix() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("outer").join("game")).unwrap();
        std::fs::create_dir_all(root.join("outer").join("inner").join("game")).unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("outer")).unwrap())
            .unwrap();
        ws.register_mod(Uri::from_file_path(root.join("outer").join("inner")).unwrap())
            .unwrap();

        let file = Uri::from_file_path(root.join("outer").join("inner").join("game").join("a.xs")).unwrap();
        let owner = ws.lookup_mod(&file).expect("owned");
        assert_eq!(
            owner.mod_path,
            root.join("outer")
                .join("inner")
                .canonicalize()
                .unwrap_or_else(|_| root.join("outer").join("inner"))
        );
    }

    #[test]
    fn lookup_mod_returns_none_for_unowned_file() {
        let tmp = TempDir::new().unwrap();
        let ws = Workspace::new(tmp.path().to_path_buf());
        let file = Uri::from_file_path("/tmp/orphan.xs").unwrap();
        assert!(ws.lookup_mod(&file).is_none());
    }

    #[test]
    fn build_virtual_project_lists_mod_overrides() {
        let tmp = TempDir::new().unwrap();
        let _game_root = tmp.path().join("game");
        let mod_root = tmp.path().join("mod_a");
        touch(
            &mod_root
                .join("game")
                .join("ai")
                .join("human_assist")
                .join("human_assist.xs"),
        )
        .unwrap();

        let mut ws = Workspace::new(tmp.path().to_path_buf());
        ws.register_mod(Uri::from_file_path(&mod_root).unwrap()).unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        assert!(project.file_overrides.contains_key("ai/human_assist/human_assist.xs"));
    }

    #[test]
    fn mod_overlay_hides_vanilla_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let vanilla = root.join("game").join("ai").join("core").join("core.xs");
        let overlay = root.join("mod_a").join("game").join("ai").join("core").join("core.xs");
        write(&vanilla, "void vanilla() {}").unwrap();
        write(&overlay, "void overlay() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        let resolved = ws.resolve_file(&project, "ai/core/core.xs").unwrap();
        assert_eq!(resolved, overlay);
        assert_ne!(resolved, vanilla);
    }

    #[test]
    fn resolve_include_prefers_mod_overlay() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let vanilla = root.join("game").join("ai").join("core").join("core.xs");
        let overlay = root.join("mod_a").join("game").join("ai").join("core").join("core.xs");
        write(&vanilla, "void vanillaCore() {}").unwrap();
        write(&overlay, "void modCore() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        let resolved = ws
            .resolve_include(&project, "ai/human_assist/human_assist.xs", "core/core.xs")
            .unwrap();
        assert_eq!(resolved, overlay);
    }

    #[test]
    fn resolve_include_falls_back_to_vanilla() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let vanilla = root.join("game").join("ai").join("core").join("core.xs");
        write(&vanilla, "void vanillaCore() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        let resolved = ws
            .resolve_include(&project, "ai/human_assist/human_assist.xs", "core/core.xs")
            .unwrap();
        assert_eq!(resolved, vanilla);
    }

    #[test]
    fn resolve_include_in_trigger_context() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let overlay = root
            .join("mod_a")
            .join("game")
            .join("data")
            .join("trigger")
            .join("bar")
            .join("bar.xs");
        write(&overlay, "void bar() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        let resolved = ws
            .resolve_include(&project, "data/trigger/foo.xs", "bar/bar.xs")
            .unwrap();
        assert_eq!(resolved, overlay);
    }

    #[test]
    fn resolve_plain_filename_in_include_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let overlay = root.join("mod_a").join("game").join("ai").join("core.xs");
        write(&overlay, "void core() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        let resolved = ws
            .resolve_include(&project, "ai/human_assist/human_assist.xs", "core.xs")
            .unwrap();
        assert_eq!(resolved, overlay);
    }

    #[test]
    fn nested_game_directory_is_not_a_separate_overlay_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nested = root.join("mod_a").join("game").join("submod").join("game").join("a.xs");
        touch(&nested).unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        // The nested game/ directory is skipped during overlay scanning.
        assert!(project.file_overrides.is_empty());
    }

    #[test]
    fn unregister_mod_removes_entry() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let mut ws = Workspace::new(root.to_path_buf());
        let uri = Uri::from_file_path(root.join("mod_a")).unwrap();
        ws.register_mod(uri.clone()).unwrap();
        assert_eq!(ws.mods().len(), 1);
        ws.unregister_mod(&uri);
        assert!(ws.mods().is_empty());
    }

    #[test]
    fn direct_include_resolves_via_tree_sitter() {
        // Scenario 4: an includer references an include target using an
        // AST-based walker, not a line regex.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let includer_rel = "ai/human_assist/a.xs";
        let includer = root.join("game").join("ai").join("human_assist").join("a.xs");
        let target = root.join("game").join("ai").join("b.xs");
        write(&includer, "include \"b.xs\";\n").unwrap();
        write(&target, "void helper() {}").unwrap();

        let ws = Workspace::new(root.to_path_buf());
        let project = VirtualProject::default();

        let source = std::fs::read_to_string(&includer).unwrap();
        let tree = parser::parse(&source).unwrap();
        let directives = parser::extract_include_directives(&tree, &source);
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].0, "b.xs");

        let resolved = ws.resolve_include(&project, includer_rel, "b.xs").unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn include_edge_prefers_mod_overlay() {
        // Scenario 8: include resolution prefers the mod overlay over vanilla.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let includer = root
            .join("mod_a")
            .join("game")
            .join("ai")
            .join("human_assist")
            .join("a.xs");
        let vanilla = root.join("game").join("ai").join("b.xs");
        let overlay = root.join("mod_a").join("game").join("ai").join("b.xs");
        write(&includer, "include \"b.xs\";\n").unwrap();
        write(&vanilla, "void vanilla() {}").unwrap();
        write(&overlay, "void overlay() {}").unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();
        let project = ws.build_virtual_project(ws.mods().first().unwrap());

        let (resolved, edge) = ws
            .resolve_include_edge(&project, "ai/human_assist/a.xs", "b.xs", &includer, 0)
            .unwrap();
        assert_eq!(resolved, overlay);
        assert_eq!(edge.to, overlay);
        assert_eq!(edge.root, IncludeRoot::Ai);
    }

    /// Regression test for the user's bug:
    ///   failed to build semantic project for .../human_assist.xs:
    ///     reading file for parse cache key: ".../game/art/ArtAtlantean.bar"
    ///
    /// The shipped AoM:R game folder contains binary `.bar` archives
    /// (`game/art/ArtAtlantean.bar`, `game/ui/UI.bar`, etc.). The previous
    /// `walk_game_files` and `collect_overlay_files` returned EVERY file
    /// in `game/` regardless of extension; `semantic::load_from_workspace`
    /// then walked them all and tried to compute a cache key by reading
    /// each as UTF-8 text. The first `.bar` it hit crashed the semantic
    /// pipeline. The fix filters by `.xs` extension at every entry point.
    #[test]
    fn visible_files_excludes_non_xs_artifacts() {
        let tmp = TempDir::new().unwrap();
        let game_root = tmp.path().join("game");
        std::fs::create_dir_all(game_root.join("art")).unwrap();
        std::fs::create_dir_all(game_root.join("ui")).unwrap();
        std::fs::create_dir_all(game_root.join("ai")).unwrap();
        std::fs::create_dir_all(game_root.join("data")).unwrap();

        // Real XS file we want to see.
        touch(&game_root.join("ai/human_assist.xs")).unwrap();

        // Binary artefacts that ship with AoM:R — these were the crash trigger.
        std::fs::write(game_root.join("art/ArtAtlantean.bar"), b"\x00\x01\x02BINARY").unwrap();
        std::fs::write(game_root.join("ui/UI.bar"), b"\x00\x01\x02BINARY").unwrap();

        // Other non-XS files that aren't `.bar` but also shouldn't be parsed.
        std::fs::write(game_root.join("art/strings.xml"), b"<xml/>").unwrap();
        std::fs::write(game_root.join("data/table.dtt"), b"\x00\x01\x02").unwrap();
        std::fs::write(game_root.join("image.png"), b"\x89PNG").unwrap();

        let ws = Workspace::new(tmp.path().to_path_buf());
        let project = VirtualProject::default();
        let files = project.visible_files(&ws);

        let rels: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(rels, vec!["ai/human_assist.xs"], "visible_files must only return .xs files; got {:?}", rels);
    }

    #[test]
    fn collect_overlay_files_excludes_non_xs_artifacts() {
        let tmp = TempDir::new().unwrap();
        let overlay_root = tmp.path().join("mod").join("game");
        std::fs::create_dir_all(overlay_root.join("ai")).unwrap();

        // Real overlay file.
        touch(&overlay_root.join("ai/auto_repair.xs")).unwrap();
        // A mod could in theory drop a binary asset into the overlay; the
        // walker must NOT pick it up as XS source.
        std::fs::write(overlay_root.join("ai/custom.bar"), b"\x00\x01\x02").unwrap();
        std::fs::write(overlay_root.join("icon.png"), b"\x89PNG").unwrap();

        let mut out = HashMap::new();
        collect_overlay_files(&overlay_root, &overlay_root, &mut out);
        let rels: std::collections::HashSet<&str> = out.keys().map(String::as_str).collect();
        assert!(rels.contains("ai/auto_repair.xs"));
        assert!(!rels.contains("ai/custom.bar"), "binary .bar must not be collected");
        assert!(!rels.contains("icon.png"), "non-XS files must not be collected");
    }

    #[test]
    fn resolve_file_rejects_binary_artifacts() {
        let tmp = TempDir::new().unwrap();
        let game_root = tmp.path().join("game");
        std::fs::create_dir_all(game_root.join("art")).unwrap();

        // A .bar file that exists on disk at the path the include walker
        // might resolve. resolve_file must refuse to return it even though
        // it exists, because it's not XS source.
        std::fs::write(game_root.join("art/ArtAtlantean.bar"), b"\x00\x01\x02").unwrap();

        let ws = Workspace::new(tmp.path().to_path_buf());
        let project = VirtualProject::default();
        let resolved = ws.resolve_file(&project, "art/ArtAtlantean.bar");
        assert!(
            resolved.is_none(),
            "resolve_file must return None for .bar even when the file exists; got {:?}",
            resolved
        );
    }

    #[test]
    fn is_xs_file_recognises_case_insensitive_extension() {
        // On Windows, AoM:R uses lowercase `.xs`; on macOS the user might
        // double-click a file with `.XS`. Either should be accepted.
        assert!(is_xs_file(Path::new("/x/y/foo.xs")));
        assert!(is_xs_file(Path::new("/x/y/foo.XS")));
        assert!(is_xs_file(Path::new("/x/y/Foo.Xs")));
        assert!(!is_xs_file(Path::new("/x/y/foo.bar")));
        assert!(!is_xs_file(Path::new("/x/y/foo")));
        assert!(!is_xs_file(Path::new("/x/y/foo.txt")));
    }

    /// Regression guard for `is_readable_xs_file`: a `.xs` file with valid
    /// UTF-8 contents (the normal case for engine XS source) MUST be
    /// accepted; a `.xs` file whose first 64 KiB are invalid UTF-8 (the
    /// AoM:R random-map serialised binary case) MUST be rejected.
    #[test]
    fn is_readable_xs_file_rejects_binary_xs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("binary.xs");
        std::fs::write(&path, [0xffu8, 0xfe, 0x00, 0xab, 0xcd]).unwrap();
        assert!(!is_readable_xs_file(&path), "binary .xs file must be rejected by is_readable_xs_file");
    }

    #[test]
    fn is_readable_xs_file_accepts_text_xs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("text.xs");
        std::fs::write(&path, "// a normal XS file\nvoid helper() {}\n").unwrap();
        assert!(is_readable_xs_file(&path), "text .xs file must be accepted by is_readable_xs_file");
    }

    /// A `.xs` file that cannot be opened at all (e.g. broken symlink, race
    /// with deletion) must not crash the walker — it must be silently
    /// rejected. The walker relies on this for resilience.
    #[test]
    fn is_readable_xs_file_rejects_nonexistent_path() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does_not_exist.xs");
        assert!(!is_readable_xs_file(&path));
    }

    /// End-to-end resilience guard for `walk_game_files`: a fake game
    /// folder containing a mix of valid and binary `.xs` files MUST return
    /// only the valid ones. This is the regression test for the user's
    /// "LSP can't give up on every single bad file" complaint — without
    /// the UTF-8 head probe, every binary `.xs` would have crashed the
    /// semantic pipeline.
    #[test]
    fn walk_game_files_skips_binary_xs() {
        let tmp = TempDir::new().unwrap();
        let game_root = tmp.path().join("game");
        std::fs::create_dir_all(game_root.join("ai")).unwrap();
        std::fs::create_dir_all(game_root.join("random_maps")).unwrap();

        // Valid text XS file in `ai/`.
        let good = game_root.join("ai").join("human_assist.xs");
        std::fs::write(&good, "void helper() {}\n").unwrap();

        // Binary `.xs` random-map file (mimicking the AoM:R random_maps/ layout).
        let bad = game_root.join("random_maps").join("aso_grasslands.xs");
        std::fs::write(&bad, [0xffu8, 0xfe, 0x00, 0xab, 0xcd, 0xef, 0x01]).unwrap();

        // Empty `.xs` file (0 bytes — `from_utf8(&[])` is Ok). Still accepted
        // by `is_readable_xs_file` since the head probe reads 0 bytes which
        // trivially decodes as UTF-8. The walker returns it; an empty file
        // is not an LSP-breaking input.
        let empty = game_root.join("ai").join("empty.xs");
        std::fs::write(&empty, b"").unwrap();

        let overrides = HashMap::new();
        let files = walk_game_files(&game_root, &overrides);
        let rels: std::collections::HashSet<String> = files.iter().map(|(r, _)| r.clone()).collect();

        assert!(rels.contains("ai/human_assist.xs"), "valid .xs file must be present, got {rels:?}");
        assert!(rels.contains("ai/empty.xs"), "empty .xs file is accepted by the head probe, got {rels:?}");
        assert!(!rels.contains("random_maps/aso_grasslands.xs"), "binary .xs file must be skipped, got {rels:?}");
    }
}
