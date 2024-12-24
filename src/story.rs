use std::error::Error;
use serde::{Deserialize, Serialize};
use log::{info, error};
use reqwest::Client;
use tokio::time::Duration;

// Constants for GPT-4 API
const GPT_API_TIMEOUT: u64 = 30;
const MAX_STORY_LENGTH: usize = 280; // Twitter character limit
const TEMPERATURE: f32 = 0.7;

// GPT API request structure
#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: i32,
    presence_penalty: f32,
    frequency_penalty: f32,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

// GPT API response structure
#[derive(Debug, Deserialize)]
struct GPTResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: MessageResponse,
    finish_reason: String,
}

#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: i32,
    prompt_tokens: i32,
    completion_tokens: i32,
}

// Main story generator
pub struct StoryGenerator {
    client: Client,
    api_key: String,
    api_endpoint: String,
}

impl StoryGenerator {
    // Initialize story generator
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("GPT4_API_KEY")
            .map_err(|_| "Missing GPT4_API_KEY environment variable")?;
            
        let client = Client::builder()
            .timeout(Duration::from_secs(GPT_API_TIMEOUT))
            .build()?;

        Ok(StoryGenerator {
            client,
            api_key,
            api_endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
        })
    }

    // Generate story based on keywords and image description
    pub async fn generate_story(
        &self,
        keywords: &[String],
        character_traits: &[String],
    ) -> Result<String, Box<dyn Error>> {
        info!("Generating story with keywords: {:?}", keywords);

        let prompt = self.build_prompt(keywords, character_traits);
        let request = self.build_request(&prompt);
        let response = self.send_request(request).await?;
        let story = self.process_response(response)?;

        info!("Story generation completed successfully");
        Ok(story)
    }

    // Build story generation prompt
    fn build_prompt(&self, keywords: &[String], character_traits: &[String]) -> String {
        format!(
            "Create a short, cheerful story (max {} characters) about a cat with these traits: {}. \
            Include these elements: {}. \
            The story should be child-friendly and end positively. \
            Focus on fun and adventure.",
            MAX_STORY_LENGTH,
            character_traits.join(", "),
            keywords.join(", ")
        )
    }

    // Build GPT API request
    fn build_request(&self, prompt: &str) -> GPTRequest {
        GPTRequest {
            model: "gpt-4".to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a creative children's story writer. \
                             Keep stories short, positive, and engaging.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: TEMPERATURE,
            max_tokens: (MAX_STORY_LENGTH as f32 * 1.5) as i32, // Allow some buffer for processing
            presence_penalty: 0.6,
            frequency_penalty: 0.5,
        }
    }

    // Send request to GPT API
    async fn send_request(&self, request: GPTRequest) -> Result<GPTResponse, Box<dyn Error>> {
        let response = self.client
            .post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("GPT API error: {}", error_text);
            return Err(StoryError::ApiError(error_text).into());
        }

        let gpt_response = response.json::<GPTResponse>().await?;
        Ok(gpt_response)
    }

    // Process GPT API response
    fn process_response(&self, response: GPTResponse) -> Result<String, Box<dyn Error>> {
        let story = response.choices
            .first()
            .ok_or(StoryError::NoStoryGenerated)?
            .message.content.clone();

        // Ensure story fits within Twitter limit
        let processed_story = self.format_story(&story)?;
        
        Ok(processed_story)
    }

    // Format and validate story
    fn format_story(&self, story: &str) -> Result<String, Box<dyn Error>> {
        let mut processed = story.trim().to_string();
        
        // Remove any hashtags or mentions
        processed = processed.replace(|c: char| c == '@' || c == '#', "");
        
        // Ensure story fits within character limit
        if processed.len() > MAX_STORY_LENGTH {
            processed = processed.chars()
                .take(MAX_STORY_LENGTH - 3)
                .collect::<String>() + "...";
        }

        // Validate final story
        if processed.is_empty() {
            return Err(StoryError::InvalidStory.into());
        }

        Ok(processed)
    }
}

// Custom error types for story operations
#[derive(Debug, thiserror::Error)]
pub enum StoryError {
    #[error("No story was generated")]
    NoStoryGenerated,
    
    #[error("Invalid story content")]
    InvalidStory,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Story generation failed: {0}")]
    GenerationFailed(String),
}

// Story generation configuration
#[derive(Debug, Clone)]
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
            style: "cheerful and adventurous".to_string(),
        }
    }
}

// Cache key generator for stories
pub fn generate_cache_key(keywords: &[String], traits: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    let combined = format!("{}-{}", keywords.join("-"), traits.join("-"));
    combined.hash(&mut hasher);
    format!("story_{:x}", hasher.finish())
}
