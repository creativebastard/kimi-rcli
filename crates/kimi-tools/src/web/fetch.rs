//! FetchURL tool - fetch a web page and extract main text content.

use crate::{Tool, ToolError, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Parameters for the FetchURL tool.
#[derive(Debug, Deserialize)]
pub struct FetchURLParams {
    /// The URL to fetch content from.
    pub url: String,
}

/// Tool for fetching web pages.
pub struct FetchURLTool {
    client: reqwest::Client,
}

impl FetchURLTool {
    /// Create a new FetchURLTool instance.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (compatible; Kimi-CLI/1.0)")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Extract main text content from HTML.
    fn extract_text(&self, html: &str) -> String {
        // Simple HTML to text extraction
        // In a real implementation, this would use a proper HTML parser
        // like readability-rs or similar

        let mut text = String::new();
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;

        let mut chars = html.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                in_tag = true;
                // Check for script or style tags
                let tag_start: String = chars.by_ref().take(6).collect();
                if tag_start.to_lowercase().starts_with("script") {
                    in_script = true;
                } else if tag_start.to_lowercase().starts_with("style") {
                    in_style = true;
                } else if tag_start.to_lowercase().starts_with("/script") {
                    in_script = false;
                } else if tag_start.to_lowercase().starts_with("/style") {
                    in_style = false;
                }
            } else if ch == '>' {
                in_tag = false;
            } else if !in_tag && !in_script && !in_style {
                text.push(ch);
            }
        }

        // Clean up whitespace
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Fetch content from a URL.
    async fn fetch(&self, url: &str) -> Result<String, ToolError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::new(format!("Failed to fetch URL '{url}': {e}")))?;

        // Check status
        if !response.status().is_success() {
            return Err(ToolError::new(format!(
                "HTTP error {} for URL: {url}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html");

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::new(format!("Failed to read response body: {e}")))?;

        // Extract text if HTML
        if content_type.contains("text/html") {
            Ok(self.extract_text(&body))
        } else {
            // Return as-is for other content types
            Ok(body)
        }
    }
}

impl Default for FetchURLTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FetchURLTool {
    fn name(&self) -> &str {
        "FetchURL"
    }

    fn description(&self) -> &str {
        "Fetch a web page from a URL and extract main text content from it."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: FetchURLParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        // Validate URL
        if !params.url.starts_with("http://") && !params.url.starts_with("https://") {
            return Err(ToolError::new(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        // Fetch the content
        let content = self.fetch(&params.url).await?;

        Ok(ToolOutput::new(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_url() {
        let tool = FetchURLTool::new();
        assert_eq!(tool.name(), "FetchURL");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_extract_text() {
        let tool = FetchURLTool::new();
        let html = "<html><body><p>Hello World</p></body></html>";
        let text = tool.extract_text(html);
        assert!(text.contains("Hello World"));
    }
}
