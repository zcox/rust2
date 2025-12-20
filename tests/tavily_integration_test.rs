//! Integration tests for the Tavily search tool
//!
//! These tests use the real Tavily API and require a valid TAVILY_API_KEY
//! environment variable to be set. They are marked with #[ignore] to prevent
//! them from running during normal test runs (to save API credits).
//!
//! Run these tests explicitly with:
//!   cargo test tavily_integration_test -- --ignored --nocapture

use rust2::llm::tools::builtin::tavily_search::{
    tavily_search, SearchDepth, SearchTopic, TavilySearchArgs,
};

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_basic_search() {
    // Load .env file for API key
    dotenvy::dotenv().ok();

    // Verify API key is set
    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set. This test requires a valid API key.");
    }

    let args = TavilySearchArgs {
        query: "Rust programming language".to_string(),
        max_results: Some(3),
        topic: None,
        search_depth: None,
        include_answer: None,
    };

    let result = tavily_search(args).await;

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());

    let search_result = result.unwrap();
    assert_eq!(search_result.query, "Rust programming language");
    assert!(!search_result.results.is_empty(), "Should return results");
    assert!(search_result.results.len() <= 3, "Should respect max_results");
    assert!(search_result.response_time > 0.0, "Should have response time");

    println!("\n=== Basic Search Results ===");
    println!("Query: {}", search_result.query);
    println!("Results: {}", search_result.results.len());
    println!("Response time: {:.2}s", search_result.response_time);
    for (i, result) in search_result.results.iter().enumerate() {
        println!("\n{}. {} (score: {:.2})", i + 1, result.title, result.score);
        println!("   URL: {}", result.url);
        println!("   {}", result.content);
    }
}

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_with_answer() {
    dotenvy::dotenv().ok();

    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set");
    }

    let args = TavilySearchArgs {
        query: "What is Rust?".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::General),
        search_depth: Some(SearchDepth::Basic),
        include_answer: Some(true),
    };

    let result = tavily_search(args).await;

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());

    let search_result = result.unwrap();
    assert!(
        search_result.answer.is_some(),
        "Should include AI-generated answer"
    );

    println!("\n=== Search with Answer ===");
    println!("Query: {}", search_result.query);
    if let Some(answer) = &search_result.answer {
        println!("Answer: {}", answer);
    }
    println!("Results: {}", search_result.results.len());
}

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_news_topic() {
    dotenvy::dotenv().ok();

    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set");
    }

    let args = TavilySearchArgs {
        query: "latest technology news".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::News),
        search_depth: Some(SearchDepth::Basic),
        include_answer: None,
    };

    let result = tavily_search(args).await;

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());

    let search_result = result.unwrap();
    assert!(!search_result.results.is_empty(), "News search should return results");

    println!("\n=== News Topic Search ===");
    println!("Query: {}", search_result.query);
    println!("Results: {}", search_result.results.len());
    for result in &search_result.results {
        println!("  - {}", result.title);
    }
}

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_advanced_depth() {
    dotenvy::dotenv().ok();

    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set");
    }

    let args = TavilySearchArgs {
        query: "machine learning frameworks comparison".to_string(),
        max_results: Some(10),
        topic: Some(SearchTopic::General),
        search_depth: Some(SearchDepth::Advanced),
        include_answer: Some(true),
    };

    let result = tavily_search(args).await;

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());

    let search_result = result.unwrap();
    assert!(!search_result.results.is_empty(), "Advanced search should return results");
    assert!(
        search_result.results.len() <= 10,
        "Should respect max_results"
    );

    println!("\n=== Advanced Depth Search ===");
    println!("Query: {}", search_result.query);
    println!("Results: {}", search_result.results.len());
    println!("Response time: {:.2}s", search_result.response_time);

    if let Some(answer) = &search_result.answer {
        println!("\nAI Answer:");
        println!("{}", answer);
    }
}

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_finance_topic() {
    dotenvy::dotenv().ok();

    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set");
    }

    let args = TavilySearchArgs {
        query: "stock market trends".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::Finance),
        search_depth: Some(SearchDepth::Basic),
        include_answer: None,
    };

    let result = tavily_search(args).await;

    assert!(result.is_ok(), "Search should succeed: {:?}", result.err());

    let search_result = result.unwrap();

    println!("\n=== Finance Topic Search ===");
    println!("Query: {}", search_result.query);
    println!("Results: {}", search_result.results.len());
    for result in &search_result.results {
        println!("  - {} (score: {:.2})", result.title, result.score);
    }
}

#[tokio::test]
#[ignore] // Run explicitly to save API credits
async fn test_real_api_max_results_limits() {
    dotenvy::dotenv().ok();

    if std::env::var("TAVILY_API_KEY").is_err() {
        panic!("TAVILY_API_KEY environment variable not set");
    }

    // Test different max_results values
    let test_cases = vec![1, 5, 10, 20];

    for max_results in test_cases {
        let args = TavilySearchArgs {
            query: "test query".to_string(),
            max_results: Some(max_results),
            topic: None,
            search_depth: None,
            include_answer: None,
        };

        let result = tavily_search(args).await;
        assert!(result.is_ok(), "Search should succeed for max_results={}", max_results);

        let search_result = result.unwrap();
        assert!(
            search_result.results.len() <= max_results as usize,
            "Results should not exceed max_results={}", max_results
        );

        println!(
            "max_results={}: got {} results",
            max_results,
            search_result.results.len()
        );

        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

#[tokio::test]
async fn test_missing_api_key_error() {
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

    assert!(result.is_err(), "Should fail without API key");
    assert!(
        result.unwrap_err().contains("TAVILY_API_KEY"),
        "Error should mention missing API key"
    );

    // Restore the API key if it existed
    if let Some(key) = original_key {
        std::env::set_var("TAVILY_API_KEY", key);
    }
}
