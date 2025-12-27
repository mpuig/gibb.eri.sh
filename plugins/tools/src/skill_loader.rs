//! Skill loader for discovering and loading SKILL.md files.
//!
//! Scans skill directories and creates GenericSkillTool instances.

use crate::skill_tool::GenericSkillTool;
use gibberish_skills::{parse_skill, SkillDefinition, SkillError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Loaded skill with its tools.
pub struct LoadedSkill {
    pub definition: SkillDefinition,
    pub tools: Vec<GenericSkillTool>,
}

/// Result of loading skills from a directory.
pub struct LoadResult {
    /// Successfully loaded skills.
    pub skills: Vec<LoadedSkill>,
    /// Errors encountered during loading.
    pub errors: Vec<SkillError>,
}

/// Load skills from the application's skills directory.
///
/// Looks for SKILL.md files in:
/// 1. Bundled skills: <app_dir>/skills/
/// 2. User skills: ~/.config/gibberish/skills/
pub fn load_all_skills() -> LoadResult {
    let mut all_skills = Vec::new();
    let mut all_errors = Vec::new();

    // Load bundled skills
    if let Some(bundled_dir) = get_bundled_skills_dir() {
        let result = load_skills_from_directory(&bundled_dir);
        all_skills.extend(result.skills);
        all_errors.extend(result.errors);
    }

    // Load user skills
    if let Some(user_dir) = get_user_skills_dir() {
        let result = load_skills_from_directory(&user_dir);
        all_skills.extend(result.skills);
        all_errors.extend(result.errors);
    }

    info!(
        skill_count = all_skills.len(),
        tool_count = all_skills.iter().map(|s| s.tools.len()).sum::<usize>(),
        error_count = all_errors.len(),
        "Skills loaded"
    );

    LoadResult {
        skills: all_skills,
        errors: all_errors,
    }
}

/// Load skills from a specific directory.
///
/// Expects directory structure:
/// ```text
/// skills/
///   git/
///     SKILL.md
///   summarize/
///     SKILL.md
/// ```
pub fn load_skills_from_directory(dir: &Path) -> LoadResult {
    let mut skills = Vec::new();
    let mut errors = Vec::new();

    if !dir.exists() {
        debug!(path = %dir.display(), "Skills directory does not exist");
        return LoadResult { skills, errors };
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            error!(path = %dir.display(), error = %e, "Failed to read skills directory");
            return LoadResult { skills, errors };
        }
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        // Look for SKILL.md in the directory
        let skill_file = path.join("SKILL.md");
        if !skill_file.exists() {
            debug!(path = %path.display(), "Skipping directory without SKILL.md");
            continue;
        }

        match load_skill(&skill_file) {
            Ok(loaded) => {
                info!(
                    skill = %loaded.definition.name,
                    tools = loaded.tools.len(),
                    "Loaded skill"
                );
                skills.push(loaded);
            }
            Err(e) => {
                warn!(path = %skill_file.display(), error = %e, "Failed to load skill");
                errors.push(e);
            }
        }
    }

    LoadResult { skills, errors }
}

/// Load a single skill from a SKILL.md file.
pub fn load_skill(path: &Path) -> Result<LoadedSkill, SkillError> {
    let definition = parse_skill(path)?;
    let tools = GenericSkillTool::from_skill(&definition);

    Ok(LoadedSkill { definition, tools })
}

/// Get the bundled skills directory.
///
/// Returns the skills/ directory relative to the executable or current directory.
fn get_bundled_skills_dir() -> Option<PathBuf> {
    // Try relative to current directory first (for development)
    let cwd_skills = PathBuf::from("skills");
    if cwd_skills.exists() {
        return Some(cwd_skills);
    }

    // Try relative to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // On macOS, the executable is in Contents/MacOS/
            // Resources are in Contents/Resources/
            let resources_dir = exe_dir.parent().and_then(|p| p.parent());
            if let Some(resources) = resources_dir {
                let bundled = resources.join("Resources").join("skills");
                if bundled.exists() {
                    return Some(bundled);
                }
            }

            // Fallback to skills/ next to executable
            let exe_skills = exe_dir.join("skills");
            if exe_skills.exists() {
                return Some(exe_skills);
            }
        }
    }

    None
}

/// Get the user skills directory.
///
/// Platform-specific paths:
/// - macOS: ~/Library/Application Support/gibb.eri.sh/skills/
/// - Linux: ~/.config/gibb.eri.sh/skills/
/// - Windows: %APPDATA%/gibb.eri.sh/skills/
fn get_user_skills_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|config| config.join("gibb.eri.sh").join("skills"))
}

/// Manager for loaded skills with reload capability.
pub struct SkillManager {
    /// All loaded skills indexed by name.
    skills: HashMap<String, LoadedSkill>,
    /// All tools indexed by name.
    tools: HashMap<String, Arc<GenericSkillTool>>,
}

impl SkillManager {
    /// Create a new skill manager and load all skills.
    pub fn new() -> Self {
        let mut manager = Self {
            skills: HashMap::new(),
            tools: HashMap::new(),
        };
        manager.reload();
        manager
    }

    /// Reload all skills from disk.
    pub fn reload(&mut self) -> ReloadResult {
        let result = load_all_skills();

        // Clear existing
        self.skills.clear();
        self.tools.clear();

        // Index loaded skills
        for loaded in result.skills {
            let skill_name = loaded.definition.name.clone();

            // Index tools
            for tool in &loaded.tools {
                self.tools
                    .insert(tool.name.to_string(), Arc::new(tool.clone()));
            }

            self.skills.insert(skill_name, loaded);
        }

        ReloadResult {
            skill_count: self.skills.len(),
            tool_count: self.tools.len(),
            errors: result.errors,
        }
    }

    /// Get a skill by name.
    pub fn get_skill(&self, name: &str) -> Option<&LoadedSkill> {
        self.skills.get(name)
    }

    /// Get a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<Arc<GenericSkillTool>> {
        self.tools.get(name).cloned()
    }

    /// Get all tool names.
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }

    /// Get all skills.
    pub fn skills(&self) -> impl Iterator<Item = &LoadedSkill> {
        self.skills.values()
    }

    /// Get skill count.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// Get tool count.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SkillManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillManager")
            .field("skills", &self.skills.keys().collect::<Vec<_>>())
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Result of reloading skills.
#[derive(Debug)]
pub struct ReloadResult {
    pub skill_count: usize,
    pub tool_count: usize,
    pub errors: Vec<SkillError>,
}

// GenericSkillTool needs Clone for the manager
impl Clone for GenericSkillTool {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            event_name: self.event_name.clone(),
            modes: self.modes.clone(),
            read_only: self.read_only,
            always_ask: self.always_ask,
            timeout_secs: self.timeout_secs,
            tool_def: Arc::clone(&self.tool_def),
            skill_name: self.skill_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_user_skills_dir() {
        let dir = get_user_skills_dir();
        // Should return some path on any system with a config dir
        assert!(dir.is_some() || std::env::var("HOME").is_err());
    }

    #[test]
    fn test_load_nonexistent_directory() {
        let result = load_skills_from_directory(Path::new("/nonexistent/path"));
        assert!(result.skills.is_empty());
        assert!(result.errors.is_empty());
    }
}
