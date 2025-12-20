//! Tavily Search Tool Demo
//!
//! This example demonstrates using the Tavily search tool with various parameters.
//!
//! Usage:
//!   cargo run --example tavily_search_demo
//!
//! Make sure to set the TAVILY_API_KEY environment variable before running.

use rust2::llm::tools::builtin::tavily_search::{
    tavily_search, SearchDepth, SearchTopic, TavilySearchArgs,
};

#[tokio::main]
async fn main() {
    // Initialize tracing for better output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load .env file
    dotenvy::dotenv().ok();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║            Tavily Search Tool Demo                            ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Check if API key is set
    if std::env::var("TAVILY_API_KEY").is_err() {
        eprintln!("❌ Error: TAVILY_API_KEY environment variable not set");
        eprintln!("   Please set it in your .env file or export it:");
        eprintln!("   export TAVILY_API_KEY=your-api-key-here");
        std::process::exit(1);
    }

    println!("✅ API key found\n");

    // Demo 1: Basic search
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 1: Basic Search                                          │");
    println!("└────────────────────────────────────────────────────────────────┘");
    demo_basic_search().await;

    println!("\n");

    // Demo 2: Search with AI answer
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 2: Search with AI-Generated Answer                       │");
    println!("└────────────────────────────────────────────────────────────────┘");
    demo_with_answer().await;

    println!("\n");

    // Demo 3: News search
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 3: News Topic Search                                     │");
    println!("└────────────────────────────────────────────────────────────────┘");
    demo_news_search().await;

    println!("\n");

    // Demo 4: Advanced depth search
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 4: Advanced Depth Search                                 │");
    println!("└────────────────────────────────────────────────────────────────┘");
    demo_advanced_search().await;

    println!("\n");

    // Demo 5: Finance search
    println!("┌────────────────────────────────────────────────────────────────┐");
    println!("│ Demo 5: Finance Topic Search                                  │");
    println!("└────────────────────────────────────────────────────────────────┘");
    demo_finance_search().await;

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║            All demos completed!                                ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
}

async fn demo_basic_search() {
    let args = TavilySearchArgs {
        query: "Rust programming language benefits".to_string(),
        max_results: Some(3),
        topic: None,
        search_depth: None,
        include_answer: None,
    };

    println!("Query: '{}'", args.query);
    println!("Parameters: max_results=3, defaults for topic/depth/answer\n");

    match tavily_search(args).await {
        Ok(result) => {
            print_results(&result);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

async fn demo_with_answer() {
    let args = TavilySearchArgs {
        query: "What are the key features of Rust?".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::General),
        search_depth: Some(SearchDepth::Basic),
        include_answer: Some(true),
    };

    println!("Query: '{}'", args.query);
    println!("Parameters: max_results=5, topic=general, depth=basic, include_answer=true\n");

    match tavily_search(args).await {
        Ok(result) => {
            if let Some(answer) = &result.answer {
                println!("🤖 AI Answer:");
                println!("   {}\n", answer);
            }
            print_results(&result);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

async fn demo_news_search() {
    let args = TavilySearchArgs {
        query: "latest technology news".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::News),
        search_depth: Some(SearchDepth::Basic),
        include_answer: None,
    };

    println!("Query: '{}'", args.query);
    println!("Parameters: max_results=5, topic=news, depth=basic\n");

    match tavily_search(args).await {
        Ok(result) => {
            print_results(&result);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

async fn demo_advanced_search() {
    let args = TavilySearchArgs {
        query: "async programming patterns".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::General),
        search_depth: Some(SearchDepth::Advanced),
        include_answer: Some(true),
    };

    println!("Query: '{}'", args.query);
    println!("Parameters: max_results=5, topic=general, depth=advanced, include_answer=true\n");

    match tavily_search(args).await {
        Ok(result) => {
            if let Some(answer) = &result.answer {
                println!("🤖 AI Answer:");
                println!("   {}\n", answer);
            }
            print_results(&result);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

async fn demo_finance_search() {
    let args = TavilySearchArgs {
        query: "cryptocurrency market trends".to_string(),
        max_results: Some(5),
        topic: Some(SearchTopic::Finance),
        search_depth: Some(SearchDepth::Basic),
        include_answer: None,
    };

    println!("Query: '{}'", args.query);
    println!("Parameters: max_results=5, topic=finance, depth=basic\n");

    match tavily_search(args).await {
        Ok(result) => {
            print_results(&result);
        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
        }
    }
}

fn print_results(result: &rust2::llm::tools::builtin::tavily_search::TavilySearchResult) {
    println!("📊 Results: {} found (response time: {:.2}s)", result.results.len(), result.response_time);
    println!();

    for (i, item) in result.results.iter().enumerate() {
        println!("{}. {} (score: {:.2})", i + 1, item.title, item.score);
        println!("   🔗 {}", item.url);

        // Wrap content at 70 characters for better readability
        let content = &item.content;
        let wrapped = wrap_text(content, 70);
        for line in wrapped {
            println!("   {}", line);
        }
        println!();
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}
