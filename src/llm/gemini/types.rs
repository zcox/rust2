//! Gemini-specific request and response types
//!
//! These types map directly to the Vertex AI Gemini API schema.

use serde::{Deserialize, Serialize};

/// Request to generate content from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentRequest {
    /// Array of content items representing the conversation
    pub contents: Vec<Content>,
    /// Optional system instruction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<SystemInstruction>,
    /// Available tools for the model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Generation configuration parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
}

/// System instruction for the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstruction {
    /// Parts of the system instruction
    pub parts: Vec<Part>,
}

/// A single content item in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Content {
    /// Role: "user" or "model"
    pub role: String,
    /// Parts of the content (may be empty when hitting limits like MAX_TOKENS)
    #[serde(default)]
    pub parts: Vec<Part>,
}

/// A part of content (text, function call, or function response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    /// Text content
    Text { text: String },
    /// Function call from the model
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
    },
    /// Function response from the application
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

/// A function call made by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Name of the function to call
    pub name: String,
    /// Arguments as a JSON object
    pub args: serde_json::Value,
}

/// A function response from the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    /// Name of the function that was called
    pub name: String,
    /// Response data as a JSON object
    pub response: serde_json::Value,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// Function declarations available to the model
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// A function declaration describing a tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    /// Function name
    pub name: String,
    /// Function description
    pub description: String,
    /// Parameters schema (JSON Schema)
    pub parameters: serde_json::Value,
}

/// Generation configuration for Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    /// Maximum number of output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p for nucleus sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k for top-k sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// Response from Gemini's streaming endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContentResponse {
    /// Candidates (usually just one)
    pub candidates: Vec<Candidate>,
    /// Usage metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<UsageMetadata>,
}

/// A candidate response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The generated content
    pub content: Content,
    /// Why the candidate finished
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Safety ratings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// Safety rating for content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    /// Category of the rating
    pub category: String,
    /// Probability of harm
    pub probability: String,
}

/// Usage metadata from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    /// Number of tokens in the prompt
    pub prompt_token_count: u32,
    /// Number of tokens in the response
    pub candidates_token_count: u32,
    /// Total token count
    pub total_token_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_part_serialization() {
        let part = Part::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"text\""));
        assert!(json.contains("\"Hello\""));
    }

    #[test]
    fn test_function_call_serialization() {
        let part = Part::FunctionCall {
            function_call: FunctionCall {
                name: "get_weather".to_string(),
                args: serde_json::json!({"location": "SF"}),
            },
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"functionCall\""));
        assert!(json.contains("\"get_weather\""));
    }

    #[test]
    fn test_function_response_serialization() {
        let part = Part::FunctionResponse {
            function_response: FunctionResponse {
                name: "get_weather".to_string(),
                response: serde_json::json!({"temperature": 72}),
            },
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"functionResponse\""));
        assert!(json.contains("\"temperature\""));
    }

    #[test]
    fn test_content_serialization() {
        let content = Content {
            role: "user".to_string(),
            parts: vec![Part::Text {
                text: "Hello".to_string(),
            }],
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"parts\""));
    }

    #[test]
    fn test_generation_config_serialization() {
        let config = GeminiGenerationConfig {
            max_output_tokens: Some(1024),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            stop_sequences: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"maxOutputTokens\":1024"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(!json.contains("\"stopSequences\""));
    }

    #[test]
    fn test_generate_content_request_serialization() {
        let request = GenerateContentRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: "Hello".to_string(),
                }],
            }],
            system_instruction: None,
            tools: None,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: Some(1024),
                temperature: None,
                top_p: None,
                top_k: None,
                stop_sequences: None,
            }),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"contents\""));
        assert!(json.contains("\"generationConfig\""));
        assert!(!json.contains("\"systemInstruction\""));
    }

    #[test]
    fn test_generate_content_response_deserialization() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello!"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        }"#;
        let response: GenerateContentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(response.candidates[0].content.role, "model");
        assert_eq!(response.usage_metadata.as_ref().unwrap().total_token_count, 15);
    }

    #[test]
    fn test_tool_declaration_serialization() {
        let tool = Tool {
            function_declarations: vec![FunctionDeclaration {
                name: "get_weather".to_string(),
                description: "Get the current weather".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                }),
            }],
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("\"functionDeclarations\""));
        assert!(json.contains("\"get_weather\""));
    }
}
