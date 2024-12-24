use log::{info, error};
use rig_core::prelude::*;
use rig_core::providers::openai::{Client as OpenAIClient, ChatRequest, ChatMessage, Role};
use serde::{Deserialize, Serialize};

const MAX_STORY_LENGTH: usize = 280; // Twitter character limit
const TEMPERATURE: f32 = 0.7;

pub struct StoryGenerator {
    openai_client: OpenAIClient,
    config: StoryConfig,
}

impl StoryGenerator {
    pub fn new(openai_client: &OpenAIClient) -> Result<Self, StoryError> {
        Ok(StoryGenerator {
            openai_client: openai_client.clone(),
            config: StoryConfig::default(),
        })
    }

    pub async fn generate_story(&self, keywords: &[String]) -> Result<String, StoryError> {
        info!("Generating story with keywords: {:?}", keywords);

        let prompt = self.build_prompt(keywords);
        let request = self.build_request(&prompt);
        
        let response = self.openai_client
            .chat(request)
            .await
            .map_err(|e| StoryError::ApiError(e.to_string()))?;

        let story = response
            .choices
            .first()
            .ok_or(StoryError::NoStoryGenerated)?
            .message
            .content
            .clone();

        let formatted_story = self.format_story(&story)?;
        
        info!("Story generation completed successfully");
        Ok(formatted_story)
    }

    fn build_prompt(&self, keywords: &[String]) -> String {
        format!(
            "Create a short, {} story (max {} characters) about a cat. \
            Include these elements: {}. \
            The story should be child-friendly and end positively. \
            Focus on fun and adventure.",
            self.config.style,
            self.config.max_length,
            keywords.join(", ")
        )
    }

    fn build_request(&self, prompt: &str) -> ChatRequest {
        ChatRequest::new()
            .model("gpt-4")
            .temperature(self.config.temperature)
            .max_tokens((self.config.max_length as f32 * 1.5) as i32)
            .presence_penalty(0.6)
            .frequency_penalty(0.5)
            .messages(vec![
                ChatMessage {
                    role: Role::System,
                    content: "You are a creative children's story writer. Keep stories short, positive, and engaging.".into(),
                    name: None,
                },
                ChatMessage {
                    role: Role::User,
                    content: prompt.into(),
                    name: None,
                },
            ])
    }

    fn format_story(&self, story: &str) -> Result<String, StoryError> {
        let mut processed = story.trim().to_string();
        
        // Remove any hashtags or mentions
        processed = processed.replace(|c: char| c == '@' || c == '#', "");
        
        // Ensure story fits within character limit
        if processed.len() > self.config.max_length {
            processed = processed.chars()
                .take(self.config.max_length - 3)
                .collect::<String>() + "...";
        }

        // Validate final story
        if processed.is_empty() {
            return Err(StoryError::InvalidStory);
        }

        Ok(processed)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoryError {
    #[error("No story was generated")]
    NoStoryGenerated,
    
    #[error("Invalid story content")]
    InvalidStory,
    
    #[error("API error: {0}")]
    ApiError(String),
}

impl From<StoryError> for rig_core::Error {
    fn from(err: StoryError) -> Self {
        rig_core::Error::Provider(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryConfig {
    pub max_length: usize,
    pub temperature: f32,
    pub style: String,
}

impl Default for StoryConfig {
    fn default() -> Self {
        StoryConfig {
            max_length: MAX_STORY_LENGTH,
            temperature: TEMPERATURE,
            style: String::from("cheerful and adventurous"),
        }
    }
}

pub fn generate_cache_key(keywords: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    keywords.join("-").hash(&mut hasher);
    format!("story_{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_generator() -> StoryGenerator {
        let openai_client = OpenAIClient::from_env().unwrap();
        StoryGenerator::new(&openai_client).unwrap()
    }

    #[test]
    fn test_story_formatting() {
        let generator = setup_test_generator();
        let story = "Hello @user! This is a #test story.";
        let formatted = generator.format_story(story).unwrap();
        assert!(!formatted.contains('@'));
        assert!(!formatted.contains('#'));
    }

    #[test]
    fn test_story_length_limit() {
        let generator = setup_test_generator();
        let long_story = "a".repeat(MAX_STORY_LENGTH + 100);
        let formatted = generator.format_story(&long_story).unwrap();
        assert!(formatted.len() <= MAX_STORY_LENGTH);
        assert!(formatted.ends_with("..."));
    }
}
