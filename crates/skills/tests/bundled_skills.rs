//! Integration tests for bundled skills.
//!
//! Validates that all SKILL.md files in the skills/ directory parse correctly.

use gibberish_skills::{parse_skill, SkillError};
use std::path::Path;

/// Get the path to the bundled skills directory.
fn skills_dir() -> std::path::PathBuf {
    // From crates/skills/tests/, go up to repo root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent() // crates/
        .unwrap()
        .parent() // repo root
        .unwrap()
        .join("skills")
}

/// Find all SKILL.md files in the bundled skills directory.
fn find_skill_files() -> Vec<std::path::PathBuf> {
    let skills_path = skills_dir();
    let mut files = Vec::new();

    if !skills_path.exists() {
        return files;
    }

    for entry in std::fs::read_dir(&skills_path).unwrap() {
        let entry = entry.unwrap();
        let skill_file = entry.path().join("SKILL.md");
        if skill_file.exists() {
            files.push(skill_file);
        }
    }

    files.sort();
    files
}

#[test]
fn test_all_bundled_skills_parse() {
    let skill_files = find_skill_files();

    assert!(
        !skill_files.is_empty(),
        "No bundled skills found in {:?}",
        skills_dir()
    );

    println!("Found {} bundled skills to validate", skill_files.len());

    let mut errors: Vec<(std::path::PathBuf, SkillError)> = Vec::new();

    for skill_file in &skill_files {
        match parse_skill(skill_file) {
            Ok(skill) => {
                println!(
                    "✓ {} - {} tools ({})",
                    skill.name,
                    skill.tools.len(),
                    skill_file.display()
                );

                // Additional validation
                assert!(
                    !skill.tools.is_empty(),
                    "Skill {} has no tools",
                    skill.name
                );

                for tool in &skill.tools {
                    assert!(
                        !tool.name.is_empty(),
                        "Skill {} has a tool with empty name",
                        skill.name
                    );
                    assert!(
                        !tool.command.program.is_empty(),
                        "Tool {} in skill {} has no command program",
                        tool.name,
                        skill.name
                    );
                }
            }
            Err(e) => {
                errors.push((skill_file.clone(), e));
            }
        }
    }

    if !errors.is_empty() {
        let error_msgs: Vec<String> = errors
            .iter()
            .map(|(path, err)| format!("  - {}: {}", path.display(), err))
            .collect();

        panic!(
            "Failed to parse {} bundled skill(s):\n{}",
            errors.len(),
            error_msgs.join("\n")
        );
    }

    println!(
        "\n✓ All {} bundled skills validated successfully",
        skill_files.len()
    );
}

#[test]
fn test_git_skill() {
    let skill_file = skills_dir().join("git/SKILL.md");
    if !skill_file.exists() {
        println!("Skipping: git skill not found");
        return;
    }

    let skill = parse_skill(&skill_file).expect("Failed to parse git skill");

    assert_eq!(skill.name, "git");
    assert!(skill.read_only, "git skill should be read-only");
    assert!(
        skill.tools.len() >= 3,
        "git skill should have at least 3 tools"
    );

    // Verify expected tools exist
    let tool_names: Vec<&str> = skill.tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"git_status"), "Missing git_status tool");
    assert!(tool_names.contains(&"git_log"), "Missing git_log tool");
    assert!(tool_names.contains(&"git_diff"), "Missing git_diff tool");
}

#[test]
fn test_files_skill() {
    let skill_file = skills_dir().join("files/SKILL.md");
    if !skill_file.exists() {
        println!("Skipping: files skill not found");
        return;
    }

    let skill = parse_skill(&skill_file).expect("Failed to parse files skill");

    assert_eq!(skill.name, "files");
    assert!(skill.read_only, "files skill should be read-only");
    assert!(
        skill.tools.len() >= 3,
        "files skill should have at least 3 tools"
    );
}

#[test]
fn test_summarize_skill() {
    let skill_file = skills_dir().join("summarize/SKILL.md");
    if !skill_file.exists() {
        println!("Skipping: summarize skill not found");
        return;
    }

    let skill = parse_skill(&skill_file).expect("Failed to parse summarize skill");

    assert_eq!(skill.name, "summarize");
    assert!(skill.read_only, "summarize skill should be read-only");
    assert!(
        skill.tools.len() >= 2,
        "summarize skill should have at least 2 tools"
    );
}

#[test]
fn test_skill_tools_have_valid_commands() {
    let skill_files = find_skill_files();

    for skill_file in skill_files {
        let skill = parse_skill(&skill_file).unwrap();

        for tool in &skill.tools {
            // Command program should not be empty
            assert!(
                !tool.command.program.is_empty(),
                "Tool {}.{} has empty command program",
                skill.name,
                tool.name
            );

            // Command program should not contain shell metacharacters
            let forbidden_chars = ['|', '&', ';', '`', '$', '(', ')', '<', '>'];
            for ch in &forbidden_chars {
                assert!(
                    !tool.command.program.contains(*ch),
                    "Tool {}.{} command program contains forbidden char '{}'",
                    skill.name,
                    tool.name,
                    ch
                );
            }
        }
    }
}
