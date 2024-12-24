use log::{info, error};
use rig::completion::{Completion, Message}; 
use rig::providers::openai::Client;  
use serde::{Serialize, Deserialize};
use base64::prelude::*;
use anyhow::Result;

const DEFAULT_STYLE: &str = "children's book illustration style";

pub struct ImageGenerator {
    openai_client: Client,
    config: ImageConfig,
}

#[derive(Debug, Clone, Serialize)]
struct ImageGenerationRequest {
    prompt: String,
    n: i32,
    size: String,
    response_format: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ImageGenerationResponse {
    created: u64,
    data: Vec<ImageData>,
}

#[derive(Debug, Deserialize, Serialize)]  // 添加 Serialize
struct ImageData {
    b64_json: String,
}

impl ImageGenerator {
    pub fn new(openai_client: &Client) -> Result<Self> {
        Ok(ImageGenerator {
            openai_client: openai_client.clone(),
            config: ImageConfig::default(),
        })
    }

    pub async fn generate_cat_image(&self, keywords: &[String]) -> Result<Vec<u8>, ImageError> {
        info!("Generating cat image with keywords: {:?}", keywords);

        let prompt = self.build_prompt(keywords);
        
        let agent = self.openai_client
            .agent("dall-e-3")
            .build();
        
        let messages = vec![Message {
            role: "user".to_string(),
            content: format!(
                "Generate an image: {}. Return the image data in base64 format.",
                prompt
            ),
        }];

        let completion_result = agent
            .completion(&messages[0].content, messages)
            .await
            .map_err(|e| ImageError::ApiError(e.to_string()))?;

        let response_content = completion_result.content
            .map_err(|e| ImageError::ApiError(e.to_string()))?;

        let temp_response = ImageGenerationResponse {
            created: chrono::Utc::now().timestamp() as u64,
            data: vec![ImageData {
                b64_json: response_content,
            }],
        };

        let json_str = serde_json::to_string(&temp_response)
            .map_err(|e| ImageError::ProcessingError(e.to_string()))?;
            
        let image_data = self.process_response(&json_str)?;
        
        // Validate the generated image
        if !self.validate_image(&image_data)? {
            return Err(ImageError::InvalidImageFormat);
        }
        
        info!("Image generation completed successfully");
        Ok(image_data)
    }

    fn build_prompt(&self, keywords: &[String]) -> String {
        let keywords_str = keywords.join(", ");
        format!(
            "A cute cartoon cat with characteristics of {}, drawn in {}, \
            warm colors, friendly expression, simple background, safe for children",
            keywords_str,
            self.config.style
        )
    }

    fn process_response(&self, response: &str) -> Result<Vec<u8>, ImageError> {
        let parsed_response = serde_json::from_str::<ImageGenerationResponse>(response)
            .map_err(|e| ImageError::ProcessingError(e.to_string()))?;
        
        let image_data = parsed_response.data
            .first()
            .ok_or(ImageError::NoImageGenerated)?;

        BASE64_STANDARD.decode(&image_data.b64_json)
            .map_err(|e| ImageError::ProcessingError(e.to_string()))
    }

    pub fn validate_image(&self, image_data: &[u8]) -> Result<bool, ImageError> {
        if image_data.is_empty() {
            return Ok(false);
        }

        // Check for JPEG magic numbers
        if image_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Ok(true);
        }

        // Check for PNG magic numbers
        if image_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            return Ok(true);
        }

        Ok(false)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("No image was generated")]
    NoImageGenerated,
    
    #[error("Invalid image format")]
    InvalidImageFormat,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Processing error: {0}")]
    ProcessingError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub size: String,
    pub style: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig {
            size: "1024x1024".to_string(), // Updated to DALL-E 3 default size
            style: String::from(DEFAULT_STYLE),
        }
    }
}

pub fn generate_cache_key(keywords: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    keywords.join("-").hash(&mut hasher);
    format!("img_{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_generator() -> ImageGenerator {
        let openai_client = Client::from_env().unwrap();
        ImageGenerator::new(&openai_client).unwrap()
    }

    #[test]
    fn test_prompt_building() {
        let generator = setup_test_generator();
        let keywords = vec!["playful".to_string(), "fluffy".to_string()];
        let prompt = generator.build_prompt(&keywords);
        assert!(prompt.contains("playful"));
        assert!(prompt.contains("fluffy"));
        assert!(prompt.contains("cartoon cat"));
    }

    #[test]
    fn test_image_validation() {
        let generator = setup_test_generator();
        
        // Test empty data
        assert!(!generator.validate_image(&[]).unwrap());
        
        // Test valid JPEG header
        assert!(generator.validate_image(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap());
        
        // Test valid PNG header
        assert!(generator.validate_image(&[0x89, 0x50, 0x4E, 0x47]).unwrap());
    }
}
