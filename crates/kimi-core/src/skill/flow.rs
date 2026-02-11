//! Flow diagram parser for flow-type skills
//! 
//! Supports Mermaid and D2 diagram formats.

use std::collections::HashMap;

/// Flow for flow-type skills
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub nodes: HashMap<String, FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub begin_id: String,
    pub end_id: String,
}

/// A node in a flow diagram
#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
}

/// Type of flow node
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    /// Entry point of the flow
    Begin,
    /// Exit point of the flow
    End,
    /// Task to be executed
    Task,
    /// Decision point with branches
    Decision,
}

/// An edge connecting two nodes in a flow
#[derive(Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

impl Flow {
    /// Create a new empty flow
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            begin_id: String::new(),
            end_id: String::new(),
        }
    }

    /// Parse a flow diagram from markdown content
    /// 
    /// Automatically detects Mermaid or D2 format based on code block language.
    pub fn parse(content: &str) -> Result<Self, String> {
        // Look for mermaid code block
        if let Some(mermaid) = extract_code_block(content, "mermaid") {
            return Self::parse_mermaid(&mermaid);
        }
        
        // Look for d2 code block
        if let Some(d2) = extract_code_block(content, "d2") {
            return Self::parse_d2(&d2);
        }
        
        Err("No flow diagram found (expected mermaid or d2 code block)".to_string())
    }

    /// Parse a Mermaid flowchart diagram
    /// 
    /// Example:
    /// ```mermaid
    /// flowchart TD
    ///     Begin([Begin]) --> Task1[Do something]
    ///     Task1 --> Decision{Is valid?}
    ///     Decision -->|Yes| End([End])
    ///     Decision -->|No| Task1
    /// ```
    pub fn parse_mermaid(content: &str) -> Result<Self, String> {
        let mut flow = Flow::new();
        let mut found_begin = false;
        let mut found_end = false;

        for line in content.lines() {
            let line = line.trim();
            
            // Skip empty lines and flowchart declaration
            if line.is_empty() || line.starts_with("flowchart") {
                continue;
            }
            
            // Parse node definitions and edges
            // Format: NodeId[Label] or NodeId{Label} or NodeId([Label])
            if let Some((from, to, edge_label)) = parse_mermaid_edge(line) {
                // Extract node info from the line
                let from_node = extract_mermaid_node(line, &from)?;
                let to_node = extract_mermaid_node(line, &to)?;
                
                // Add nodes
                if from_node.node_type == NodeType::Begin {
                    flow.begin_id = from_node.id.clone();
                    found_begin = true;
                }
                if to_node.node_type == NodeType::Begin {
                    flow.begin_id = to_node.id.clone();
                    found_begin = true;
                }
                if from_node.node_type == NodeType::End {
                    flow.end_id = from_node.id.clone();
                    found_end = true;
                }
                if to_node.node_type == NodeType::End {
                    flow.end_id = to_node.id.clone();
                    found_end = true;
                }
                
                flow.nodes.entry(from_node.id.clone()).or_insert(from_node);
                flow.nodes.entry(to_node.id.clone()).or_insert(to_node);
                
                // Add edge
                flow.edges.push(FlowEdge {
                    from,
                    to,
                    label: edge_label,
                });
            }
        }

        if !found_begin {
            return Err("Flow must have a Begin node".to_string());
        }
        if !found_end {
            return Err("Flow must have an End node".to_string());
        }

        Ok(flow)
    }

    /// Parse a D2 diagram
    /// 
    /// Example:
    /// ```d2
    /// Begin: {
    ///   shape: circle
    ///   label: Begin
    /// }
    /// Task1: Do something
    /// Decision: {
    ///   shape: diamond
    ///   label: Is valid?
    /// }
    /// 
    /// Begin -> Task1
    /// Task1 -> Decision
    /// Decision -> End: Yes
    /// Decision -> Task1: No
    /// ```
    pub fn parse_d2(content: &str) -> Result<Self, String> {
        let mut flow = Flow::new();
        let mut found_begin = false;
        let mut found_end = false;
        let mut in_block = false;
        let mut block_lines: Vec<&str> = Vec::new();
        let mut block_start_id: Option<String> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Handle multi-line block definitions
            if in_block {
                block_lines.push(trimmed);
                if trimmed == "}" {
                    // End of block
                    if let Some(id) = block_start_id.take() {
                        if let Some(node) = parse_d2_block(&id, &block_lines) {
                            if node.node_type == NodeType::Begin {
                                flow.begin_id = node.id.clone();
                                found_begin = true;
                            }
                            if node.node_type == NodeType::End {
                                flow.end_id = node.id.clone();
                                found_end = true;
                            }
                            flow.nodes.insert(node.id.clone(), node);
                        }
                    }
                    in_block = false;
                    block_lines.clear();
                }
                continue;
            }

            // Check for start of block definition: "Name: {"
            if trimmed.ends_with('{') && trimmed.contains(':') && !trimmed.contains("->") {
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 && parts[1].trim() == "{" {
                    in_block = true;
                    block_start_id = Some(parts[0].trim().to_string());
                    block_lines.push(trimmed);
                    continue;
                }
            }

            // Parse single-line node definitions
            if trimmed.contains(':') && !trimmed.contains("->") {
                if let Some(node) = parse_d2_node(trimmed) {
                    if node.node_type == NodeType::Begin {
                        flow.begin_id = node.id.clone();
                        found_begin = true;
                    }
                    if node.node_type == NodeType::End {
                        flow.end_id = node.id.clone();
                        found_end = true;
                    }
                    flow.nodes.insert(node.id.clone(), node);
                }
            }

            // Parse edges: From -> To: Label
            if let Some((from, to, label)) = parse_d2_edge(trimmed) {
                flow.edges.push(FlowEdge { from, to, label });
            }
        }

        if !found_begin {
            return Err("Flow must have a Begin node".to_string());
        }
        if !found_end {
            return Err("Flow must have an End node".to_string());
        }

        Ok(flow)
    }
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract content from a code block with given language
fn extract_code_block(content: &str, language: &str) -> Option<String> {
    let pattern = format!("```{}\n", language);
    let start = content.find(&pattern)?;
    let content_start = start + pattern.len();
    let end = content[content_start..].find("```")?;
    Some(content[content_start..content_start + end].to_string())
}

/// Parse a Mermaid edge line: A --> B or A -->|Label| B
fn parse_mermaid_edge(line: &str) -> Option<(String, String, Option<String>)> {
    // Remove comments
    let line = line.split("%%").next()?.trim();
    
    // Find the arrow
    if let Some(arrow_pos) = line.find("-->") {
        let before = &line[..arrow_pos].trim();
        let after = &line[arrow_pos + 3..].trim();
        
        // Check for labeled arrow: -->|Label|
        let (label, after_arrow): (Option<String>, &str) = if let Some(stripped) = after.strip_prefix('|') {
            let label_end = stripped.find('|')?;
            let label = stripped[..label_end].to_string();
            (Some(label), stripped[label_end + 1..].trim())
        } else {
            (None, after)
        };
        
        // Extract node IDs (before any node shape markers)
        let from_id = extract_node_id(before);
        let to_id = extract_node_id(after_arrow);
        
        return Some((from_id, to_id, label));
    }
    
    None
}

/// Extract node ID from Mermaid node syntax
fn extract_node_id(s: &str) -> String {
    let s = s.trim();
    // Handle various node shapes: [label], (label), {label}, ([label])
    if let Some(start) = s.find(['[', '(', '{']) {
        s[..start].trim().to_string()
    } else {
        s.to_string()
    }
}

/// Extract full node info from Mermaid line
fn extract_mermaid_node(line: &str, node_id: &str) -> Result<FlowNode, String> {
    // Find the node definition in the line
    let pattern = format!("{}[", node_id);
    let alt_pattern1 = format!("{}(", node_id);
    let alt_pattern2 = format!("{}{{", node_id);
    
    let (start, _end_char) = if let Some(pos) = line.find(&pattern) {
        (pos, ']')
    } else if let Some(pos) = line.find(&alt_pattern1) {
        (pos, ')')
    } else if let Some(pos) = line.find(&alt_pattern2) {
        (pos, '}')
    } else {
        // Node ID without explicit definition, treat as task
        return Ok(FlowNode {
            id: node_id.to_string(),
            label: node_id.to_string(),
            node_type: NodeType::Task,
        });
    };
    
    let node_start = start + node_id.len();
    let node_def = &line[node_start..];
    
    // Determine node type and extract label
    let (node_type, label) = if node_def.starts_with("([") {
        // Circle: ([Label])
        let end = node_def.find("])").ok_or("Unclosed circle node")?;
        let label = node_def[2..end].to_string();
        let nt = if label.eq_ignore_ascii_case("begin") {
            NodeType::Begin
        } else if label.eq_ignore_ascii_case("end") {
            NodeType::End
        } else {
            NodeType::Task
        };
        (nt, label)
    } else if let Some(stripped) = node_def.strip_prefix('[') {
        // Rectangle: [Label]
        let end = stripped.find(']').ok_or("Unclosed rectangle node")?;
        let label = stripped[..end].to_string();
        (NodeType::Task, label)
    } else if let Some(stripped) = node_def.strip_prefix('{') {
        // Diamond: {Label}
        let end = stripped.find('}').ok_or("Unclosed diamond node")?;
        let label = stripped[..end].to_string();
        (NodeType::Decision, label)
    } else if let Some(stripped) = node_def.strip_prefix('(') {
        // Rounded: (Label)
        let end = stripped.find(')').ok_or("Unclosed rounded node")?;
        let label = stripped[..end].to_string();
        let nt = if label.eq_ignore_ascii_case("begin") {
            NodeType::Begin
        } else if label.eq_ignore_ascii_case("end") {
            NodeType::End
        } else {
            NodeType::Task
        };
        (nt, label)
    } else {
        (NodeType::Task, node_id.to_string())
    };
    
    Ok(FlowNode {
        id: node_id.to_string(),
        label,
        node_type,
    })
}

/// Parse a D2 node definition
fn parse_d2_node(line: &str) -> Option<FlowNode> {
    // Simple format: "Name: Label" or "Name: { shape: ... }"
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    
    let id = parts[0].trim().to_string();
    let rest = parts[1].trim();
    
    // Check if it's a shape definition
    if rest.starts_with('{') {
        // Parse shape properties
        let content = rest.trim_start_matches('{').trim_end_matches('}');
        
        let mut label = id.clone();
        let mut node_type = NodeType::Task;
        
        for prop in content.split('\n') {
            let prop = prop.trim();
            if let Some(stripped) = prop.strip_prefix("label:") {
                label = stripped.trim().to_string();
            } else if let Some(stripped) = prop.strip_prefix("shape:") {
                let shape = stripped.trim();
                match shape {
                    "circle" | "oval" => {
                        if label.eq_ignore_ascii_case("begin") {
                            node_type = NodeType::Begin;
                        } else if label.eq_ignore_ascii_case("end") {
                            node_type = NodeType::End;
                        }
                    }
                    "diamond" => node_type = NodeType::Decision,
                    _ => node_type = NodeType::Task,
                }
            }
        }
        
        Some(FlowNode { id, label, node_type })
    } else {
        // Simple label
        let label = rest.to_string();
        let node_type = if id.eq_ignore_ascii_case("begin") {
            NodeType::Begin
        } else if id.eq_ignore_ascii_case("end") {
            NodeType::End
        } else {
            NodeType::Task
        };
        
        Some(FlowNode { id, label, node_type })
    }
}

/// Parse a D2 block definition (multi-line)
fn parse_d2_block(id: &str, lines: &[&str]) -> Option<FlowNode> {
    let mut label = id.to_string();
    let mut node_type = NodeType::Task;
    
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "{" || trimmed == "}" {
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("label:") {
            label = stripped.trim().to_string();
        } else if let Some(stripped) = trimmed.strip_prefix("shape:") {
            let shape = stripped.trim();
            match shape {
                "circle" | "oval" => {
                    if label.eq_ignore_ascii_case("begin") {
                        node_type = NodeType::Begin;
                    } else if label.eq_ignore_ascii_case("end") {
                        node_type = NodeType::End;
                    }
                }
                "diamond" => node_type = NodeType::Decision,
                _ => node_type = NodeType::Task,
            }
        }
    }
    
    Some(FlowNode {
        id: id.to_string(),
        label,
        node_type,
    })
}

/// Parse a D2 edge: "From -> To: Label" or "From -> To"
fn parse_d2_edge(line: &str) -> Option<(String, String, Option<String>)> {
    if !line.contains("->") {
        return None;
    }
    
    let parts: Vec<&str> = line.split("->").collect();
    if parts.len() != 2 {
        return None;
    }
    
    let from = parts[0].trim().to_string();
    
    // Handle optional label after colon
    let to_part = parts[1].trim();
    let (to, label) = if let Some(colon_pos) = to_part.find(':') {
        let to = to_part[..colon_pos].trim().to_string();
        let label = to_part[colon_pos + 1..].trim().to_string();
        (to, Some(label))
    } else {
        (to_part.to_string(), None)
    };
    
    Some((from, to, label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mermaid_basic() {
        let content = r#"flowchart TD
    Begin([Begin]) --> Task1[Do something]
    Task1 --> End([End])"#;
        
        let flow = Flow::parse_mermaid(content).unwrap();
        assert_eq!(flow.begin_id, "Begin");
        assert_eq!(flow.end_id, "End");
        assert_eq!(flow.nodes.len(), 3);
        assert_eq!(flow.edges.len(), 2);
    }

    #[test]
    fn test_parse_mermaid_with_decision() {
        let content = r#"flowchart TD
    Begin([Begin]) --> Check{Is valid?}
    Check -->|Yes| End([End])
    Check -->|No| Begin"#;
        
        let flow = Flow::parse_mermaid(content).unwrap();
        assert_eq!(flow.edges.len(), 3);
        
        let yes_edge = flow.edges.iter().find(|e| e.label.as_deref() == Some("Yes")).unwrap();
        assert_eq!(yes_edge.from, "Check");
        assert_eq!(yes_edge.to, "End");
    }

    #[test]
    fn test_parse_d2_basic() {
        let content = r#"Begin: {
  shape: circle
  label: Begin
}
Task1: Do something
End: {
  shape: circle
  label: End
}

Begin -> Task1
Task1 -> End"#;
        
        let flow = Flow::parse_d2(content).unwrap();
        assert_eq!(flow.begin_id, "Begin");
        assert_eq!(flow.end_id, "End");
        assert_eq!(flow.nodes.len(), 3);
        assert_eq!(flow.edges.len(), 2);
    }

    #[test]
    fn test_extract_code_block() {
        let content = r#"Some text
```mermaid
flowchart TD
    A --> B
```
More text"#;
        
        let extracted = extract_code_block(content, "mermaid").unwrap();
        assert!(extracted.contains("flowchart TD"));
        assert!(extracted.contains("A --> B"));
    }
}
