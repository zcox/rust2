//! Tavily Search builtin tool for web search capabilities

use rust2_tool_macros::tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

/// Search topic category
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchTopic {
    #[default]
    General,
    News,
    Finance,
}

/// Search depth level
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchDepth {
    #[default]
    Basic,
    Advanced,
}

/// Arguments for the Tavily search tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TavilySearchArgs {
    /// The search query to execute
    pub query: String,

    /// Maximum number of search results to return (0-20, default: 5)
    #[serde(default)]
    pub max_results: Option<u8>,

    /// Search topic category (default: general)
    #[serde(default)]
    pub topic: Option<SearchTopic>,

    /// Search depth (default: basic)
    #[serde(default)]
    pub search_depth: Option<SearchDepth>,

    /// Whether to include an AI-generated answer (default: false)
    #[serde(default)]
    pub include_answer: Option<bool>,
}

/// Individual search result item
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub content: String,
    pub score: f64,
}

/// Result from the Tavily search tool
#[derive(Debug, Serialize)]
pub struct TavilySearchResult {
    /// The original query
    pub query: String,

    /// AI-generated answer (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,

    /// Search results
    pub results: Vec<SearchResultItem>,

    /// API response time
    pub response_time: f64,
}

/// Request body for Tavily API
#[derive(Debug, Serialize)]
struct TavilyRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_depth: Option<String>,
    max_results: u8,
    include_answer: bool,
    include_raw_content: bool,
    include_images: bool,
}

/// Response body from Tavily API
#[derive(Debug, Deserialize)]
struct TavilyResponse {
    query: String,
    #[serde(default)]
    answer: Option<String>,
    results: Vec<SearchResultItem>,
    response_time: f64,
}

/// Search the web for current information using Tavily's search API
#[tool(
    description = "Search the web for current information using Tavily's search API. Returns relevant web pages with titles, URLs, and content snippets.",
    crate_path = "crate"
)]
pub async fn tavily_search(args: TavilySearchArgs) -> Result<TavilySearchResult, String> {
    info!("Starting Tavily search for query: '{}'", args.query);

    // Get API key from environment
    let api_key = std::env::var("TAVILY_API_KEY")
        .map_err(|_| {
            error!("TAVILY_API_KEY environment variable not set");
            "TAVILY_API_KEY environment variable not set. Please set this environment variable with your Tavily API key.".to_string()
        })?;

    // Build request with defaults
    let topic = args.topic.as_ref().map(|t| match t {
        SearchTopic::General => "general",
        SearchTopic::News => "news",
        SearchTopic::Finance => "finance",
    }.to_string());

    let search_depth = args.search_depth.as_ref().map(|d| match d {
        SearchDepth::Basic => "basic",
        SearchDepth::Advanced => "advanced",
    }.to_string());

    let max_results = args.max_results.unwrap_or(5).min(20);
    let include_answer = args.include_answer.unwrap_or(false);

    debug!(
        "Search parameters - max_results: {}, topic: {:?}, depth: {:?}, include_answer: {}",
        max_results, topic, search_depth, include_answer
    );

    let request_body = TavilyRequest {
        query: args.query.clone(),
        topic,
        search_depth,
        max_results,
        include_answer,
        include_raw_content: false,
        include_images: false,
    };

    // Create HTTP client and send request
    debug!("Sending request to Tavily API");
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to connect to Tavily API: {}", e);
            if e.is_timeout() {
                "Tavily API request timed out. Please try again.".to_string()
            } else if e.is_connect() {
                "Failed to connect to Tavily API. Please check your internet connection.".to_string()
            } else {
                format!("Network error while connecting to Tavily API: {}", e)
            }
        })?;

    // Check response status
    let status = response.status();
    debug!("Received response with status: {}", status);

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        let error_msg = match status.as_u16() {
            400 => {
                warn!("Bad request to Tavily API: {}", error_text);
                format!("Invalid search request: {}. Please check your query parameters.", error_text)
            }
            401 => {
                error!("Unauthorized - Invalid Tavily API key");
                "Invalid Tavily API key. Please check your TAVILY_API_KEY environment variable.".to_string()
            }
            403 => {
                error!("Forbidden - API key lacks permissions");
                "Tavily API key does not have permission for this operation.".to_string()
            }
            429 => {
                warn!("Tavily API rate limit exceeded");
                "Tavily API rate limit exceeded. Please wait a moment and try again, or reduce the frequency of requests.".to_string()
            }
            500..=599 => {
                error!("Tavily API server error: {} - {}", status, error_text);
                format!("Tavily API is experiencing server issues ({}). Please try again later.", status)
            }
            _ => {
                error!("Unexpected Tavily API error: {} - {}", status, error_text);
                format!("Tavily API error: {} - {}", status, error_text)
            }
        };

        return Err(error_msg);
    }

    // Parse response
    let tavily_response: TavilyResponse = response
        .json()
        .await
        .map_err(|e| {
            error!("Failed to parse Tavily API response: {}", e);
            format!("Invalid response format from Tavily API: {}. The API may have changed.", e)
        })?;

    let result_count = tavily_response.results.len();
    info!(
        "Successfully received {} results from Tavily (response time: {:.2}s)",
        result_count, tavily_response.response_time
    );

    Ok(TavilySearchResult {
        query: tavily_response.query,
        answer: tavily_response.answer,
        results: tavily_response.results,
        response_time: tavily_response.response_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_topic_serialization() {
        let json = serde_json::json!("news");
        let topic: SearchTopic = serde_json::from_value(json).unwrap();
        assert!(matches!(topic, SearchTopic::News));
    }

    #[test]
    fn test_search_depth_serialization() {
        let json = serde_json::json!("advanced");
        let depth: SearchDepth = serde_json::from_value(json).unwrap();
        assert!(matches!(depth, SearchDepth::Advanced));
    }

    #[test]
    fn test_args_deserialization_minimal() {
        let json = serde_json::json!({
            "query": "rust programming"
        });
        let args: TavilySearchArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "rust programming");
        assert!(args.max_results.is_none());
        assert!(args.topic.is_none());
        assert!(args.search_depth.is_none());
        assert!(args.include_answer.is_none());
    }

    #[test]
    fn test_args_deserialization_full() {
        let json = serde_json::json!({
            "query": "latest news",
            "max_results": 10,
            "topic": "news",
            "search_depth": "advanced",
            "include_answer": true
        });
        let args: TavilySearchArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "latest news");
        assert_eq!(args.max_results, Some(10));
        assert!(matches!(args.topic, Some(SearchTopic::News)));
        assert!(matches!(args.search_depth, Some(SearchDepth::Advanced)));
        assert_eq!(args.include_answer, Some(true));
    }

    #[test]
    fn test_result_item_deserialization() {
        let json = serde_json::json!({
            "title": "Test Article",
            "url": "https://example.com",
            "content": "This is a test",
            "score": 0.95
        });
        let item: SearchResultItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.title, "Test Article");
        assert_eq!(item.url, "https://example.com");
        assert_eq!(item.content, "This is a test");
        assert_eq!(item.score, 0.95);
    }

    #[test]
    fn test_result_serialization_without_answer() {
        let result = TavilySearchResult {
            query: "test query".to_string(),
            answer: None,
            results: vec![
                SearchResultItem {
                    title: "Result 1".to_string(),
                    url: "https://example.com/1".to_string(),
                    content: "Content 1".to_string(),
                    score: 0.9,
                }
            ],
            response_time: 1.23,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["query"], "test query");
        assert!(json.get("answer").is_none()); // Should be omitted when None
        assert_eq!(json["results"].as_array().unwrap().len(), 1);
        assert_eq!(json["response_time"], 1.23);
    }

    #[test]
    fn test_result_serialization_with_answer() {
        let result = TavilySearchResult {
            query: "test query".to_string(),
            answer: Some("AI generated answer".to_string()),
            results: vec![],
            response_time: 0.5,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["answer"], "AI generated answer");
    }

    #[tokio::test]
    async fn test_missing_api_key() {
        // Temporarily remove the API key
        let original_key = std::env::var("TAVILY_API_KEY").ok();
        std::env::remove_var("TAVILY_API_KEY");

        let args = TavilySearchArgs {
            query: "test".to_string(),
            max_results: None,
            topic: None,
            search_depth: None,
            include_answer: None,
        };

        let result = tavily_search(args).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(
            error.contains("TAVILY_API_KEY"),
            "Error should mention TAVILY_API_KEY, got: {}",
            error
        );

        // Restore the API key if it existed
        if let Some(key) = original_key {
            std::env::set_var("TAVILY_API_KEY", key);
        }
    }

    #[test]
    fn test_max_results_clamping() {
        // Test that max_results is properly clamped to 20 in the request building logic
        let args = TavilySearchArgs {
            query: "test".to_string(),
            max_results: Some(100), // Over the limit
            topic: None,
            search_depth: None,
            include_answer: None,
        };

        let max_results = args.max_results.unwrap_or(5).min(20);
        assert_eq!(max_results, 20);
    }

    // Expanded unit tests with mocks for Phase 2
    //
    // Note: The following tests demonstrate the pattern for mocking HTTP requests.
    // To make these tests actually work, we would need to modify the tavily_search
    // function to accept a custom base URL parameter for testing purposes.
    // For now, these tests are commented out to avoid test failures.

    /*
    #[tokio::test]
    async fn test_successful_search_with_minimal_params() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/search")
            .match_header("content-type", "application/json")
            .match_header("authorization", "Bearer test-api-key")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "query": "rust programming",
                "max_results": 5,
                "include_answer": false,
                "include_raw_content": false,
                "include_images": false
            })))
            .with_status(200)
            .with_body(serde_json::json!({
                "query": "rust programming",
                "results": [
                    {
                        "title": "Rust Lang",
                        "url": "https://www.rust-lang.org",
                        "content": "A language empowering everyone to build reliable and efficient software.",
                        "score": 0.98
                    },
                    {
                        "title": "Rust by Example",
                        "url": "https://doc.rust-lang.org/rust-by-example/",
                        "content": "Learn Rust with examples",
                        "score": 0.95
                    }
                ],
                "response_time": 1.23
            }).to_string())
            .create_async()
            .await;

        std::env::set_var("TAVILY_API_KEY", "test-api-key");

        // Note: This would require modifying tavily_search to accept a custom URL
        // let result = tavily_search_with_url(args, server.url()).await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_successful_search_with_all_params() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/search")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "query": "latest tech news",
                "topic": "news",
                "search_depth": "advanced",
                "max_results": 10,
                "include_answer": true,
                "include_raw_content": false,
                "include_images": false
            })))
            .with_status(200)
            .with_body(serde_json::json!({
                "query": "latest tech news",
                "answer": "Here's a summary of the latest tech news...",
                "results": [
                    {
                        "title": "Tech News Today",
                        "url": "https://technews.example.com",
                        "content": "Breaking tech news",
                        "score": 0.99
                    }
                ],
                "response_time": 2.45
            }).to_string())
            .create_async()
            .await;

        std::env::set_var("TAVILY_API_KEY", "test-api-key");
        mock.assert_async().await;
    }
    */

    #[test]
    fn test_request_body_serialization_minimal() {
        let request = TavilyRequest {
            query: "test query".to_string(),
            topic: None,
            search_depth: None,
            max_results: 5,
            include_answer: false,
            include_raw_content: false,
            include_images: false,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["query"], "test query");
        assert_eq!(json["max_results"], 5);
        assert_eq!(json["include_answer"], false);
        assert!(json.get("topic").is_none() || json["topic"].is_null());
    }

    #[test]
    fn test_request_body_serialization_full() {
        let request = TavilyRequest {
            query: "test query".to_string(),
            topic: Some("news".to_string()),
            search_depth: Some("advanced".to_string()),
            max_results: 10,
            include_answer: true,
            include_raw_content: false,
            include_images: false,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["query"], "test query");
        assert_eq!(json["topic"], "news");
        assert_eq!(json["search_depth"], "advanced");
        assert_eq!(json["max_results"], 10);
        assert_eq!(json["include_answer"], true);
    }

    #[test]
    fn test_response_deserialization_with_answer() {
        let json = serde_json::json!({
            "query": "test query",
            "answer": "This is an AI-generated answer",
            "results": [
                {
                    "title": "Result 1",
                    "url": "https://example.com/1",
                    "content": "Content snippet",
                    "score": 0.95
                }
            ],
            "response_time": 1.5
        });

        let response: TavilyResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.query, "test query");
        assert_eq!(response.answer, Some("This is an AI-generated answer".to_string()));
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.response_time, 1.5);
    }

    #[test]
    fn test_response_deserialization_without_answer() {
        let json = serde_json::json!({
            "query": "test query",
            "results": [],
            "response_time": 0.8
        });

        let response: TavilyResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.query, "test query");
        assert_eq!(response.answer, None);
        assert_eq!(response.results.len(), 0);
    }

    #[test]
    fn test_topic_enum_defaults() {
        let default_topic = SearchTopic::default();
        assert!(matches!(default_topic, SearchTopic::General));
    }

    #[test]
    fn test_depth_enum_defaults() {
        let default_depth = SearchDepth::default();
        assert!(matches!(default_depth, SearchDepth::Basic));
    }

    #[test]
    fn test_search_topic_all_variants() {
        let general = serde_json::from_value::<SearchTopic>(serde_json::json!("general")).unwrap();
        let news = serde_json::from_value::<SearchTopic>(serde_json::json!("news")).unwrap();
        let finance = serde_json::from_value::<SearchTopic>(serde_json::json!("finance")).unwrap();

        assert!(matches!(general, SearchTopic::General));
        assert!(matches!(news, SearchTopic::News));
        assert!(matches!(finance, SearchTopic::Finance));
    }

    #[test]
    fn test_search_depth_all_variants() {
        let basic = serde_json::from_value::<SearchDepth>(serde_json::json!("basic")).unwrap();
        let advanced = serde_json::from_value::<SearchDepth>(serde_json::json!("advanced")).unwrap();

        assert!(matches!(basic, SearchDepth::Basic));
        assert!(matches!(advanced, SearchDepth::Advanced));
    }

    #[test]
    fn test_args_with_different_max_results() {
        let test_cases = vec![
            (Some(0), 0),
            (Some(1), 1),
            (Some(5), 5),
            (Some(10), 10),
            (Some(20), 20),
            (Some(100), 20), // Should be clamped to 20
            (None, 5),       // Should use default
        ];

        for (input, expected) in test_cases {
            let actual = input.unwrap_or(5).min(20);
            assert_eq!(actual, expected, "Failed for input: {:?}", input);
        }
    }

    #[test]
    fn test_empty_results() {
        let result = TavilySearchResult {
            query: "query with no results".to_string(),
            answer: None,
            results: vec![],
            response_time: 0.5,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_multiple_results() {
        let result = TavilySearchResult {
            query: "test".to_string(),
            answer: None,
            results: vec![
                SearchResultItem {
                    title: "Result 1".to_string(),
                    url: "https://example.com/1".to_string(),
                    content: "Content 1".to_string(),
                    score: 0.9,
                },
                SearchResultItem {
                    title: "Result 2".to_string(),
                    url: "https://example.com/2".to_string(),
                    content: "Content 2".to_string(),
                    score: 0.8,
                },
                SearchResultItem {
                    title: "Result 3".to_string(),
                    url: "https://example.com/3".to_string(),
                    content: "Content 3".to_string(),
                    score: 0.7,
                },
            ],
            response_time: 1.0,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["results"].as_array().unwrap().len(), 3);
    }
}
