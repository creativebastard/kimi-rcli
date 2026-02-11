//! YAML frontmatter parser for SKILL.md files

use serde::Deserialize;

/// Parsed frontmatter from a SKILL.md file
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub skill_type: Option<String>,
}

/// Parse YAML frontmatter from markdown content
/// 
/// Frontmatter is expected to be at the start of the file between `---` markers:
/// ```markdown
/// ---
/// name: My Skill
/// description: A useful skill
/// type: flow
/// ---
/// 
/// # Content
/// ```
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, &str), String> {
    let trimmed = content.trim_start();
    
    // Check if content starts with frontmatter delimiter
    if !trimmed.starts_with("---") {
        return Ok((Frontmatter::default(), content));
    }
    
    // Find the end of frontmatter
    let after_open = &trimmed[3..];
    let Some(end_pos) = after_open.find("---") else {
        return Ok((Frontmatter::default(), content));
    };
    
    let yaml_content = &after_open[..end_pos].trim();
    let rest = &after_open[end_pos + 3..];
    
    // Parse YAML
    let frontmatter: Frontmatter = serde_yaml::from_str(yaml_content)
        .map_err(|e| format!("Failed to parse YAML frontmatter: {}", e))?;
    
    Ok((frontmatter, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = r#"---
name: Test Skill
description: A test skill
type: flow
---

# Content here
"#;
        
        let (fm, rest) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, Some("Test Skill".to_string()));
        assert_eq!(fm.description, Some("A test skill".to_string()));
        assert_eq!(fm.skill_type, Some("flow".to_string()));
        assert!(rest.contains("# Content here"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just markdown\nNo frontmatter here.";
        
        let (fm, rest) = parse_frontmatter(content).unwrap();
        assert!(fm.name.is_none());
        assert!(fm.description.is_none());
        assert!(fm.skill_type.is_none());
        assert_eq!(rest, content);
    }

    #[test]
    fn test_parse_empty_frontmatter() {
        let content = r#"---
---

# Content"#;
        
        let (fm, rest) = parse_frontmatter(content).unwrap();
        assert!(fm.name.is_none());
        assert!(rest.contains("# Content"));
    }
}
