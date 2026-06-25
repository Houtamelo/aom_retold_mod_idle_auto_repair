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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;

/// A registered mod workspace folder.
#[derive(Debug, Clone)]
pub struct ModEntry {
    /// The LSP workspace-folder URI for this mod root.
    pub mod_uri: Url,
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
        root: IncludeRoot,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::UnknownIncludeRoot => write!(f, "include target has no known include root"),
            ResolveError::NotFound { target, root } => write!(
                f,
                "include target {:?} not found under {:?} root",
                target,
                root.rel_prefix()
            ),
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
    pub fn register_mod(&mut self, mod_uri: Url) -> Result<(), WorkspaceError> {
        let mod_path = mod_uri
            .to_file_path()
            .map_err(|_| WorkspaceError::NotFileUri)?;
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
    pub fn unregister_mod(&mut self, mod_uri: &Url) {
        self.mods
            .retain(|m| m.mod_uri.as_str() != mod_uri.as_str());
    }

    /// All currently registered mods.
    pub fn mods(&self) -> &[ModEntry] {
        &self.mods
    }

    /// Return the enclosing mod entry for `file_uri`, using the longest
    /// prefix match. Returns `None` for files outside every registered mod.
    pub fn lookup_mod(&self, file_uri: &Url) -> Option<&ModEntry> {
        let path = file_uri.to_file_path().ok()?;
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
    /// Mod overlays take precedence over the vanilla game folder.
    pub fn resolve_file(&self, project: &VirtualProject, rel: &str) -> Option<PathBuf> {
        if let Some(path) = project.file_overrides.get(rel) {
            return Some(path.clone());
        }
        let game_file = self.game_path.join("game").join(rel);
        if game_file.exists() {
            Some(game_file)
        } else {
            None
        }
    }

    /// Resolve `include "target"` from `from_rel` inside `project`.
    ///
    /// The resolution uses the include-root context of `from_rel`, so an
    /// `include "core/core.xs"` from `game/ai/human_assist/human_assist.xs`
    /// first looks for `game/ai/core/core.xs` in the mod overlay, then in the
    /// vanilla game folder.
    pub fn resolve_include(
        &self,
        project: &VirtualProject,
        from_rel: &str,
        include_target: &str,
    ) -> Option<PathBuf> {
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
    pub fn game_file_path(&self, rel: &str) -> PathBuf {
        self.game_path.join("game").join(rel)
    }

    /// Relative path under `game/` for an absolute file path, if it lies inside
    /// the workspace game folder.
    pub fn game_relative_path(&self, abs_path: &Path) -> Option<String> {
        let game_root = self.game_path.join("game");
        relativize(&game_root, abs_path)
    }

    /// The game-path root used by this workspace.
    pub fn game_path(&self) -> &Path {
        &self.game_path
    }
}

/// Normalize an include target so backslashes from Windows-style paths are
/// treated as path separators on all platforms.
fn normalize_include_target(target: &str) -> String {
    target.replace('\\', "/")
}

/// Infer the include-root context for a file from its path under `game/`.
pub fn detect_include_root(rel_path: &str) -> Option<IncludeRoot> {
    const ROOTS: [IncludeRoot; 3] = [
        IncludeRoot::Ai,
        IncludeRoot::Trigger,
        IncludeRoot::RandomMap,
    ];
    ROOTS
        .iter()
        .find(|r| rel_path.starts_with(r.rel_prefix()))
        .copied()
}

fn collect_overlay_files(
    overlay_root: &Path,
    current: &Path,
    out: &mut HashMap<String, PathBuf>,
) {
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
            let Some(rel) = relativize(overlay_root, &path) else {
                continue;
            };
            out.insert(rel, path);
        }
    }
}

fn walk_game_files(
    game_root: &Path,
    overrides: &HashMap<String, PathBuf>,
) -> Vec<(String, PathBuf)> {
    fn walk(
        base: &Path,
        dir: &Path,
        out: &mut Vec<(String, PathBuf)>,
        overrides: &HashMap<String, PathBuf>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out, overrides);
            } else if path.is_file() {
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
    use super::*;
    use crate::parser;
    use std::io::Write;
    use tempfile::TempDir;

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
        assert_eq!(
            detect_include_root("data/trigger/foo.xs"),
            Some(IncludeRoot::Trigger)
        );
    }

    #[test]
    fn detect_include_root_recognises_random_map() {
        assert_eq!(
            detect_include_root("random_maps/foo.xs"),
            Some(IncludeRoot::RandomMap)
        );
    }

    #[test]
    fn detect_include_root_returns_none_for_unknown() {
        assert_eq!(detect_include_root("ui/foo.xs"), None);
    }

    #[test]
    fn register_mod_requires_file_uri() {
        let tmp = TempDir::new().unwrap();
        let mut ws = Workspace::new(tmp.path().to_path_buf());
        let uri = Url::parse("http://example.com/mod").unwrap();
        assert_eq!(ws.register_mod(uri).unwrap_err(), WorkspaceError::NotFileUri);
    }

    #[test]
    fn lookup_mod_uses_longest_prefix() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("outer").join("game")).unwrap();
        std::fs::create_dir_all(root.join("outer").join("inner").join("game")).unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Url::from_file_path(root.join("outer")).unwrap())
            .unwrap();
        ws.register_mod(Url::from_file_path(root.join("outer").join("inner")).unwrap())
            .unwrap();

        let file = Url::from_file_path(root.join("outer").join("inner").join("game").join("a.xs"))
            .unwrap();
        let owner = ws.lookup_mod(&file).expect("owned");
        assert_eq!(
            owner.mod_path,
            root.join("outer").join("inner").canonicalize().unwrap_or_else(|_| root.join("outer").join("inner"))
        );
    }

    #[test]
    fn lookup_mod_returns_none_for_unowned_file() {
        let tmp = TempDir::new().unwrap();
        let mut ws = Workspace::new(tmp.path().to_path_buf());
        let file = Url::from_file_path("/tmp/orphan.xs").unwrap();
        assert!(ws.lookup_mod(&file).is_none());
    }

    #[test]
    fn build_virtual_project_lists_mod_overrides() {
        let tmp = TempDir::new().unwrap();
        let game_root = tmp.path().join("game");
        let mod_root = tmp.path().join("mod_a");
        touch(&mod_root.join("game").join("ai").join("human_assist").join("human_assist.xs")).unwrap();

        let mut ws = Workspace::new(tmp.path().to_path_buf());
        ws.register_mod(Url::from_file_path(&mod_root).unwrap()).unwrap();

        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        assert!(project
            .file_overrides
            .contains_key("ai/human_assist/human_assist.xs"));
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        let nested = root
            .join("mod_a")
            .join("game")
            .join("submod")
            .join("game")
            .join("a.xs");
        touch(&nested).unwrap();

        let mut ws = Workspace::new(root.to_path_buf());
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
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
        let uri = Url::from_file_path(root.join("mod_a")).unwrap();
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
        ws.register_mod(Url::from_file_path(root.join("mod_a")).unwrap())
            .unwrap();
        let project = ws.build_virtual_project(ws.mods().first().unwrap());

        let (resolved, edge) = ws
            .resolve_include_edge(
                &project,
                "ai/human_assist/a.xs",
                "b.xs",
                &includer,
                0,
            )
            .unwrap();
        assert_eq!(resolved, overlay);
        assert_eq!(edge.to, overlay);
        assert_eq!(edge.root, IncludeRoot::Ai);
    }
}
