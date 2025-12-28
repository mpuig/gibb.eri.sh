//! Tool pack loader for discovering and loading .tool.json files.
//!
//! Scans tool pack directories and creates ToolPackTool instances.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{debug, error, info, warn};

use crate::tool_pack::{ToolPack, ToolPackTool};

/// Result of loading tool packs from a directory.
#[derive(Debug)]
pub struct LoadResult {
    /// Successfully loaded tool packs.
    pub packs: Vec<ToolPack>,
    /// Errors encountered during loading.
    pub errors: Vec<LoadError>,
}

/// Error loading a tool pack.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Parse error in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Validation error in {path}: {message}")]
    Validation { path: PathBuf, message: String },
}

/// Load all tool packs from standard directories.
///
/// Looks for .tool.json files in:
/// 1. Bundled tools: <app_dir>/tools/
/// 2. User tools: ~/.config/gibb.eri.sh/tools/
pub fn load_all_tool_packs() -> LoadResult {
    let mut all_packs = Vec::new();
    let mut all_errors = Vec::new();

    // Load bundled tool packs
    if let Some(bundled_dir) = get_bundled_tools_dir() {
        let result = load_tool_packs_from_directory(&bundled_dir);
        all_packs.extend(result.packs);
        all_errors.extend(result.errors);
    }

    // Load user tool packs
    if let Some(user_dir) = get_user_tools_dir() {
        let result = load_tool_packs_from_directory(&user_dir);
        all_packs.extend(result.packs);
        all_errors.extend(result.errors);
    }

    info!(
        pack_count = all_packs.len(),
        error_count = all_errors.len(),
        "Tool packs loaded"
    );

    LoadResult {
        packs: all_packs,
        errors: all_errors,
    }
}

/// Load tool packs from a specific directory.
///
/// Scans for:
/// - `*.tool.json` files directly in the directory
/// - `toolpack.json` files in subdirectories
pub fn load_tool_packs_from_directory(dir: &Path) -> LoadResult {
    let mut packs = Vec::new();
    let mut errors = Vec::new();

    if !dir.exists() {
        debug!(path = %dir.display(), "Tool packs directory does not exist");
        return LoadResult { packs, errors };
    }

    // Scan for .tool.json files
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!(path = %dir.display(), error = %e, "Failed to read tools directory");
            return LoadResult { packs, errors };
        }
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_file() {
            // Check for .tool.json extension
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".tool.json") {
                    match load_tool_pack(&path) {
                        Ok(pack) => {
                            info!(
                                name = %pack.name,
                                path = %path.display(),
                                "Loaded tool pack"
                            );
                            packs.push(pack);
                        }
                        Err(e) => {
                            warn!(path = %path.display(), error = %e, "Failed to load tool pack");
                            errors.push(e);
                        }
                    }
                }
            }
        } else if path.is_dir() {
            // Check for toolpack.json in subdirectory
            let toolpack_file = path.join("toolpack.json");
            if toolpack_file.exists() {
                match load_tool_pack(&toolpack_file) {
                    Ok(pack) => {
                        info!(
                            name = %pack.name,
                            path = %toolpack_file.display(),
                            "Loaded tool pack from directory"
                        );
                        packs.push(pack);
                    }
                    Err(e) => {
                        warn!(path = %toolpack_file.display(), error = %e, "Failed to load tool pack");
                        errors.push(e);
                    }
                }
            }
        }
    }

    LoadResult { packs, errors }
}

/// Load a single tool pack from a JSON file.
pub fn load_tool_pack(path: &Path) -> Result<ToolPack, LoadError> {
    let content = std::fs::read_to_string(path).map_err(|e| LoadError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let pack: ToolPack = serde_json::from_str(&content).map_err(|e| LoadError::Parse {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Validate required fields
    validate_tool_pack(&pack, path)?;

    Ok(pack)
}

/// Validate a tool pack has all required fields.
fn validate_tool_pack(pack: &ToolPack, path: &Path) -> Result<(), LoadError> {
    if pack.name.is_empty() {
        return Err(LoadError::Validation {
            path: path.to_path_buf(),
            message: "name is required".to_string(),
        });
    }

    if pack.description.is_empty() {
        return Err(LoadError::Validation {
            path: path.to_path_buf(),
            message: "description is required".to_string(),
        });
    }

    if pack.command.program.is_empty() {
        return Err(LoadError::Validation {
            path: path.to_path_buf(),
            message: "command.program is required".to_string(),
        });
    }

    // Validate name is valid identifier
    if !pack
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(LoadError::Validation {
            path: path.to_path_buf(),
            message: format!("name '{}' must be snake_case (alphanumeric and underscores)", pack.name),
        });
    }

    Ok(())
}

/// Get the bundled tools directory.
fn get_bundled_tools_dir() -> Option<PathBuf> {
    // Try relative to current directory (for development)
    let cwd_tools = PathBuf::from("tools");
    if cwd_tools.exists() {
        return Some(cwd_tools);
    }

    // Try common development paths (when running from apps/desktop/)
    for dev_path in &["../../tools", "../../../tools"] {
        let path = PathBuf::from(dev_path);
        if path.exists() {
            return Some(path);
        }
    }

    // Try relative to executable (for production)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // On macOS, the executable is in Contents/MacOS/
            // Resources are in Contents/Resources/
            let resources_dir = exe_dir.parent().and_then(|p| p.parent());
            if let Some(resources) = resources_dir {
                let bundled = resources.join("Resources").join("tools");
                if bundled.exists() {
                    return Some(bundled);
                }
            }

            // Fallback to tools/ next to executable
            let exe_tools = exe_dir.join("tools");
            if exe_tools.exists() {
                return Some(exe_tools);
            }
        }
    }

    None
}

/// Get the user tools directory.
///
/// Platform-specific paths:
/// - macOS: ~/Library/Application Support/gibb.eri.sh/tools/
/// - Linux: ~/.config/gibb.eri.sh/tools/
/// - Windows: %APPDATA%/gibb.eri.sh/tools/
fn get_user_tools_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|config| config.join("gibb.eri.sh").join("tools"))
}

/// Get all tool pack directories for watching.
pub fn get_tool_pack_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(bundled) = get_bundled_tools_dir() {
        dirs.push(bundled);
    }

    if let Some(user) = get_user_tools_dir() {
        // Create user directory if it doesn't exist
        if !user.exists() {
            if let Err(e) = std::fs::create_dir_all(&user) {
                warn!(path = %user.display(), error = %e, "Failed to create user tools directory");
            }
        }
        dirs.push(user);
    }

    dirs
}

/// Manager for loaded tool packs with reload capability.
pub struct ToolPackManager {
    /// All loaded tool packs indexed by name.
    packs: HashMap<String, Arc<ToolPack>>,
    /// Tool instances ready for use.
    tools: HashMap<String, ToolPackTool>,
}

impl ToolPackManager {
    /// Create a new manager and load all tool packs.
    pub fn new() -> Self {
        let mut manager = Self {
            packs: HashMap::new(),
            tools: HashMap::new(),
        };
        manager.reload();
        manager
    }

    /// Reload all tool packs from disk.
    pub fn reload(&mut self) -> ReloadResult {
        let result = load_all_tool_packs();

        // Clear existing
        self.packs.clear();
        self.tools.clear();

        // Index loaded packs
        for pack in result.packs {
            let name = pack.name.clone();
            let pack_arc = Arc::new(pack);

            // Create tool instance
            let tool = ToolPackTool::from_arc(Arc::clone(&pack_arc));
            self.tools.insert(name.clone(), tool);
            self.packs.insert(name, pack_arc);
        }

        info!(
            pack_count = self.packs.len(),
            "Tool packs reloaded"
        );

        ReloadResult {
            pack_count: self.packs.len(),
            errors: result.errors,
        }
    }

    /// Get a tool pack by name.
    pub fn get_pack(&self, name: &str) -> Option<Arc<ToolPack>> {
        self.packs.get(name).cloned()
    }

    /// Get a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<&ToolPackTool> {
        self.tools.get(name)
    }

    /// Get all tool names.
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }

    /// Get all tools.
    pub fn tools(&self) -> impl Iterator<Item = &ToolPackTool> {
        self.tools.values()
    }

    /// Get pack count.
    pub fn pack_count(&self) -> usize {
        self.packs.len()
    }
}

impl Default for ToolPackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolPackManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolPackManager")
            .field("packs", &self.packs.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Result of reloading tool packs.
#[derive(Debug)]
pub struct ReloadResult {
    pub pack_count: usize,
    pub errors: Vec<LoadError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_valid_tool_pack() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.tool.json");

        let content = r#"{
            "name": "test_tool",
            "description": "A test tool",
            "command": {
                "program": "echo",
                "args": ["hello"]
            }
        }"#;

        std::fs::write(&file_path, content).unwrap();

        let pack = load_tool_pack(&file_path).unwrap();
        assert_eq!(pack.name, "test_tool");
    }

    #[test]
    fn test_load_invalid_json() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("invalid.tool.json");

        std::fs::write(&file_path, "{ invalid json }").unwrap();

        let result = load_tool_pack(&file_path);
        assert!(matches!(result, Err(LoadError::Parse { .. })));
    }

    #[test]
    fn test_load_missing_name() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("noname.tool.json");

        let content = r#"{
            "name": "",
            "description": "Missing name",
            "command": { "program": "echo", "args": [] }
        }"#;

        std::fs::write(&file_path, content).unwrap();

        let result = load_tool_pack(&file_path);
        assert!(matches!(result, Err(LoadError::Validation { .. })));
    }

    #[test]
    fn test_load_from_directory() {
        let dir = tempdir().unwrap();

        // Create a valid tool pack
        let content = r#"{
            "name": "dir_test",
            "description": "Directory test",
            "command": { "program": "echo", "args": ["test"] }
        }"#;
        std::fs::write(dir.path().join("test.tool.json"), content).unwrap();

        // Create an invalid one (should be skipped)
        std::fs::write(dir.path().join("bad.tool.json"), "invalid").unwrap();

        let result = load_tool_packs_from_directory(dir.path());
        assert_eq!(result.packs.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.packs[0].name, "dir_test");
    }

    #[test]
    fn test_name_validation() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("badname.tool.json");

        let content = r#"{
            "name": "bad-name-with-dashes",
            "description": "Invalid name",
            "command": { "program": "echo", "args": [] }
        }"#;

        std::fs::write(&file_path, content).unwrap();

        let result = load_tool_pack(&file_path);
        assert!(matches!(result, Err(LoadError::Validation { .. })));
    }
}
