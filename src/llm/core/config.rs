//! Generation configuration parameters

use serde::{Deserialize, Serialize};

/// Parameters for controlling text generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum number of tokens to generate
    pub max_tokens: u32,
    /// Randomness (0.0-1.0, higher = more random)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Nucleus sampling threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling (Gemini-specific, ignored for Claude)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Stop generation when these sequences are encountered
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl GenerationConfig {
    /// Create a new configuration with the specified max tokens
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
        }
    }

    /// Set the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the top_p value
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// Set the top_k value (Gemini only)
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    /// Set stop sequences
    pub fn with_stop_sequences(mut self, stop_sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(stop_sequences);
        self
    }
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 1024,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config = GenerationConfig::new(2048);
        assert_eq!(config.max_tokens, 2048);
        assert!(config.temperature.is_none());
        assert!(config.top_p.is_none());
        assert!(config.top_k.is_none());
        assert!(config.stop_sequences.is_none());
    }

    #[test]
    fn test_config_default() {
        let config = GenerationConfig::default();
        assert_eq!(config.max_tokens, 1024);
    }

    #[test]
    fn test_config_builder() {
        let config = GenerationConfig::new(2048)
            .with_temperature(0.7)
            .with_top_p(0.9)
            .with_top_k(40)
            .with_stop_sequences(vec!["STOP".to_string()]);

        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.top_p, Some(0.9));
        assert_eq!(config.top_k, Some(40));
        assert_eq!(config.stop_sequences, Some(vec!["STOP".to_string()]));
    }

    #[test]
    fn test_config_serialization() {
        let config = GenerationConfig::new(1024).with_temperature(0.5);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"max_tokens\":1024"));
        assert!(json.contains("\"temperature\":0.5"));
        // Optional fields that are None should not be in the JSON
        assert!(!json.contains("\"top_p\""));
        assert!(!json.contains("\"top_k\""));
        assert!(!json.contains("\"stop_sequences\""));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{"max_tokens":2048,"temperature":0.8}"#;
        let config: GenerationConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.temperature, Some(0.8));
        assert!(config.top_p.is_none());
    }
}
