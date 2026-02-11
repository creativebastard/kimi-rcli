//! SearchWeb tool - search the internet for information.

use crate::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;

/// Parameters for the SearchWeb tool.
#[derive(Debug, Deserialize)]
pub struct SearchWebParams {
    /// The query text to search for.
    pub query: String,
    /// The number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Whether to include the content of the web pages in the results.
    #[serde(default)]
    pub include_content: bool,
}

fn default_limit() -> usize {
    5
}

/// A single search result.
#[derive(Debug, serde::Serialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub content: Option<String>,
}

/// Tool for searching the web.
#[derive(Debug)]
pub struct SearchWebTool {
    client: reqwest::Client,
    #[allow(dead_code)]
    api_endpoint: Option<String>,
    #[allow(dead_code)]
    api_key: Option<String>,
}

impl SearchWebTool {
    /// Create a new SearchWebTool with default settings.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_endpoint: None,
            api_key: None,
        }
    }

    /// Create a new SearchWebTool with API configuration.
    pub fn with_api(endpoint: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_endpoint: Some(endpoint.into()),
            api_key: Some(api_key.into()),
        }
    }

    /// Perform a web search using a search API.
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        // This is a placeholder implementation
        // In a real implementation, this would call a search API like:
        // - Google Custom Search API
        // - Bing Search API
        // - Brave Search API
        // - SerpAPI
        // - etc.

        // For now, return a mock result indicating the tool is not fully configured
        Ok(vec![SearchResult {
            title: "Search API not configured".to_string(),
            url: String::new(),
            snippet: format!(
                "The SearchWeb tool is not fully configured. \
                 Query: '{}' (limit: {})",
                query, limit
            ),
            content: None,
        }])
    }

    /// Fetch content from a URL.
    async fn fetch_content(&self, url: &str) -> Result<String, ToolError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::new(format!("Failed to fetch URL: {e}")))?;

        let content = response
            .text()
            .await
            .map_err(|e| ToolError::new(format!("Failed to read response: {e}")))?;

        // TODO: Extract main text content from HTML
        // This would typically use a library like readability-rs or similar

        Ok(content)
    }
}

impl Default for SearchWebTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SearchWebTool {
    fn name(&self) -> &str {
        "SearchWeb"
    }

    fn description(&self) -> &str {
        "Search on the internet to get latest information, including news, documents, release notes, blog posts, papers, etc."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The query text to search for"
                },
                "limit": {
                    "type": "integer",
                    "description": "The number of results to return",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 20
                },
                "include_content": {
                    "type": "boolean",
                    "description": "Whether to include the content of the web pages in the results"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> ToolResult {
        let params: SearchWebParams = serde_json::from_value(params)
            .map_err(|e| ToolError::new(format!("Invalid parameters: {e}")))?;

        // Clamp limit to valid range
        let limit = params.limit.clamp(1, 20);

        // Perform the search
        let mut results = self.search(&params.query, limit).await?;

        // Fetch content if requested
        if params.include_content {
            for result in &mut results {
                if !result.url.is_empty() {
                    match self.fetch_content(&result.url).await {
                        Ok(content) => {
                            result.content = Some(content);
                        }
                        Err(e) => {
                            result.content = Some(format!("Failed to fetch content: {e}"));
                        }
                    }
                }
            }
        }

        // Format output
        let output = if results.is_empty() {
            "No results found.".to_string()
        } else {
            serde_json::to_string_pretty(&results)
                .map_err(|e| ToolError::new(format!("Failed to serialize results: {e}")))?
        };

        Ok(serde_json::json!(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_web() {
        let tool = SearchWebTool::new();
        assert_eq!(tool.name(), "SearchWeb");
        assert!(!tool.description().is_empty());
    }
}
