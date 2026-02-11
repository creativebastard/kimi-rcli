//! Skill discovery from filesystem

use std::path::{Path, PathBuf};

use super::{Flow, Skill, SkillError, SkillType};

/// Skill discovery from filesystem
#[derive(Debug)]
pub struct SkillDiscovery;

impl SkillDiscovery {
    /// Find skills in a directory
    /// 
    /// Scans the directory for subdirectories containing SKILL.md files
    /// and parses them into Skill structs.
    pub async fn discover(dir: &Path) -> Vec<Skill> {
        let mut skills = Vec::new();
        
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return skills;
        };
        
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            
            // Skip non-directories
            if !path.is_dir() {
                continue;
            }
            
            // Check for SKILL.md
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            
            // Read and parse the file
            match tokio::fs::read_to_string(&skill_md).await {
                Ok(content) => {
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    match Self::parse_skill(&name, &content, path) {
                        Ok(skill) => skills.push(skill),
                        Err(e) => {
                            tracing::warn!("Failed to parse skill {}: {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read {:?}: {}", skill_md, e);
                }
            }
        }
        
        skills
    }
    
    /// Resolve skill roots (builtin, user, project)
    /// 
    /// Returns a list of directories to search for skills, in order of priority:
    /// 1. Project skills (.kimi/skills/)
    /// 2. User skills (~/.config/kimi/skills/)
    /// 3. Builtin skills (shipped with the application)
    pub async fn resolve_roots(work_dir: &Path) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        
        // Project skills: .kimi/skills/ in the working directory
        let project_skills = work_dir.join(".kimi").join("skills");
        if project_skills.exists() {
            roots.push(project_skills);
        }
        
        // User skills: ~/.config/kimi/skills/
        if let Some(home) = dirs::home_dir() {
            let user_skills = home.join(".config").join("kimi").join("skills");
            if user_skills.exists() {
                roots.push(user_skills);
            }
        }
        
        // Builtin skills would be resolved at runtime based on the executable location
        // This is a placeholder - actual implementation would depend on how the app is packaged
        
        roots
    }
    
    /// Parse a SKILL.md file into a Skill struct
    /// 
    /// The file should have YAML frontmatter with at least a name and description.
    /// For flow-type skills, include a flowchart diagram.
    pub fn parse_skill(name: &str, content: &str, dir: PathBuf) -> Result<Skill, SkillError> {
        use super::frontmatter::parse_frontmatter;
        
        let (frontmatter, rest) = parse_frontmatter(content)
            .map_err(SkillError::InvalidFrontmatter)?;
        
        // Use frontmatter name if provided, otherwise use directory name
        let skill_name = frontmatter.name.unwrap_or_else(|| name.to_string());
        
        // Description is required
        let description = frontmatter.description
            .ok_or_else(|| SkillError::MissingField("description".to_string()))?;
        
        // Determine skill type
        let skill_type = match frontmatter.skill_type.as_deref() {
            Some("flow") => SkillType::Flow,
            _ => SkillType::Standard,
        };
        
        // Parse flow if this is a flow-type skill
        let flow = if skill_type == SkillType::Flow {
            Some(Flow::parse(rest).map_err(SkillError::InvalidFlow)?)
        } else {
            None
        };
        
        Ok(Skill {
            name: skill_name,
            description,
            skill_type,
            dir,
            flow,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_standard() {
        let content = r#"---
name: Test Skill
description: A test skill
---

# Test Skill

This is a test skill.
"#;
        
        let skill = SkillDiscovery::parse_skill("test-skill", content, PathBuf::from("/test")).unwrap();
        assert_eq!(skill.name, "Test Skill");
        assert_eq!(skill.description, "A test skill");
        assert_eq!(skill.skill_type, SkillType::Standard);
        assert!(skill.flow.is_none());
    }

    #[test]
    fn test_parse_skill_flow() {
        let content = r#"---
name: Flow Skill
description: A flow-based skill
type: flow
---

# Flow Skill

```mermaid
flowchart TD
    Begin([Begin]) --> Task[Do something]
    Task --> End([End])
```
"#;
        
        let skill = SkillDiscovery::parse_skill("flow-skill", content, PathBuf::from("/test")).unwrap();
        assert_eq!(skill.name, "Flow Skill");
        assert_eq!(skill.description, "A flow-based skill");
        assert_eq!(skill.skill_type, SkillType::Flow);
        assert!(skill.flow.is_some());
        
        let flow = skill.flow.unwrap();
        assert_eq!(flow.begin_id, "Begin");
        assert_eq!(flow.end_id, "End");
    }

    #[test]
    fn test_parse_skill_missing_description() {
        let content = r#"---
name: Bad Skill
---

# Bad Skill
"#;
        
        let result = SkillDiscovery::parse_skill("bad-skill", content, PathBuf::from("/test"));
        assert!(matches!(result, Err(SkillError::MissingField(_))));
    }

    #[test]
    fn test_parse_skill_no_frontmatter() {
        let content = r#"# Just Markdown

No frontmatter here.
"#;
        
        // Should fail because description is missing
        let result = SkillDiscovery::parse_skill("no-frontmatter", content, PathBuf::from("/test"));
        assert!(matches!(result, Err(SkillError::MissingField(_))));
    }
}
