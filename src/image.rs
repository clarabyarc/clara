use log::{info, error};
use rig::prelude::*;
use rig::providers::openai::{Client as OpenAIClient, ImageRequest, ImageResponse, ImageSize};
use serde::{Serialize, Deserialize};
use base64::prelude::*;

const DEFAULT_STYLE: &str = "children's book illustration style";

pub struct ImageGenerator {
    openai_client: OpenAIClient,
    config: ImageConfig,
}

impl ImageGenerator {
    pub fn new(openai_client: &OpenAIClient) -> Result<Self, ImageError> {
        Ok(ImageGenerator {
            openai_client: openai_client.clone(),
            config: ImageConfig::default(),
        })
    }

    pub async fn generate_cat_image(&self, keywords: &[String]) -> Result<Vec<u8>, ImageError> {
        info!("Generating cat image with keywords: {:?}", keywords);

        let prompt = self.build_prompt(keywords);
        
        let request = ImageRequest::new()
            .prompt(&prompt)
            .n(1)
            .size(ImageSize::S512x512)
            .response_format("b64_json");

        let response = self.openai_client
            .create_image(request)
            .await
            .map_err(|e| ImageError::ApiError(e.to_string()))?;

        let image_data = self.process_response(response)?;
        
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

    fn process_response(&self, response: ImageResponse) -> Result<Vec<u8>, ImageError> {
        let image_data = response.data
            .first()
            .ok_or(ImageError::NoImageGenerated)?;

        BASE64_STANDARD.decode(&image_data.b64_json)
            .map_err(|e| ImageError::ProcessingError(e.to_string()))
    }

    pub fn validate_image(&self, image_data: &[u8]) -> Result<bool, ImageError> {
        if image_data.is_empty() {
            return Ok(false);
        }

        // Check for JPEG header
        if image_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Ok(true);
        }

        Err(ImageError::InvalidImageFormat)
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

impl From<ImageError> for rig::Error {
    fn from(err: ImageError) -> Self {
        rig::Error::Provider(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub size: ImageSize,
    pub style: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig {
            size: ImageSize::S512x512,
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
