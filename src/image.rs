use log::{info, error};
use reqwest::Client;
use tokio::time::Duration;
use base64::prelude::*;
use rig_core::Error as RigError;
use serde::{Serialize, Deserialize};

// Constants for DALL-E API
const DALLE_API_TIMEOUT: u64 = 30;
const IMAGE_SIZE: &str = "512x512";
const DEFAULT_STYLE: &str = "children's book illustration style";

// DALL-E API request structure
#[derive(Debug, Serialize)]
struct DallERequest {
    prompt: String,
    n: i32,
    size: String,
    response_format: String,
}

// DALL-E API response structure
#[derive(Debug, Deserialize)]
struct DallEResponse {
    created: u64,
    data: Vec<ImageData>,
}

#[derive(Debug, Deserialize)]
struct ImageData {
    b64_json: String,
}

// Main image generator
pub struct ImageGenerator {
    client: Client,
    api_key: String,
    api_endpoint: String,
}

impl ImageGenerator {
    // Initialize image generator
    pub fn new() -> Result<Self, ImageError> {
        let api_key = std::env::var("DALLE_API_KEY")
            .map_err(|_| ImageError::ConfigError("Missing DALLE_API_KEY environment variable".to_string()))?;
            
        let client = Client::builder()
            .timeout(Duration::from_secs(DALLE_API_TIMEOUT))
            .build()
            .map_err(|e| ImageError::ClientError(e.to_string()))?;

        Ok(ImageGenerator {
            client,
            api_key,
            api_endpoint: String::from("https://api.openai.com/v1/images/generations"),
        })
    }

    // Generate image based on keywords
    pub async fn generate_cat_image(&self, keywords: &[String]) -> Result<Vec<u8>, ImageError> {
        info!("Generating cat image with keywords: {:?}", keywords);

        let prompt = self.build_prompt(keywords);
        let request = self.build_request(&prompt);
        let response = self.send_request(request).await?;
        let image_data = self.process_response(response)?;

        info!("Image generation completed successfully");
        Ok(image_data)
    }

    // Build image generation prompt
    fn build_prompt(&self, keywords: &[String]) -> String {
        let keywords_str = keywords.join(", ");
        format!(
            "A cute cartoon cat with characteristics of {}, drawn in {}, \
            warm colors, friendly expression, simple background, safe for children",
            keywords_str,
            DEFAULT_STYLE
        )
    }

    // Build DALL-E API request
    fn build_request(&self, prompt: &str) -> DallERequest {
        DallERequest {
            prompt: String::from(prompt),
            n: 1,
            size: String::from(IMAGE_SIZE),
            response_format: String::from("b64_json"),
        }
    }

    // Send request to DALL-E API
    async fn send_request(&self, request: DallERequest) -> Result<DallEResponse, ImageError> {
        let response = self.client
            .post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| ImageError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response.text().await
                .map_err(|e| ImageError::ApiError(e.to_string()))?;
            error!("DALL-E API error: {}", error_text);
            return Err(ImageError::ApiError(error_text));
        }

        response.json::<DallEResponse>().await
            .map_err(|e| ImageError::ApiError(e.to_string()))
    }

    // Process DALL-E API response
    fn process_response(&self, response: DallEResponse) -> Result<Vec<u8>, ImageError> {
        let image_data = response.data
            .first()
            .ok_or(ImageError::NoImageGenerated)?;

        // Decode base64 image data
        BASE64_STANDARD.decode(&image_data.b64_json)
            .map_err(|e| ImageError::ProcessingError(e.to_string()))
    }

    // Apply post-processing to generated image
    pub fn post_process_image(&self, image_data: Vec<u8>) -> Result<Vec<u8>, ImageError> {
        Ok(image_data)
    }

    // Validate generated image
    pub fn validate_image(&self, image_data: &[u8]) -> Result<bool, ImageError> {
        if image_data.is_empty() {
            return Ok(false);
        }

        if image_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Ok(true);
        }

        Err(ImageError::InvalidImageFormat)
    }
}

// Custom error types for image operations
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("No image was generated")]
    NoImageGenerated,
    
    #[error("Invalid image format")]
    InvalidImageFormat,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Client error: {0}")]
    ClientError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Processing error: {0}")]
    ProcessingError(String),
}

// Implement conversion from ImageError to RigError
impl From<ImageError> for RigError {
    fn from(err: ImageError) -> RigError {
        RigError::Generic(err.to_string())
    }
}

// Image generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub size: String,
    pub style: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig {
            size: String::from(IMAGE_SIZE),
            style: String::from(DEFAULT_STYLE),
        }
    }
}

// Cache key generator for images
pub fn generate_cache_key(keywords: &[String]) -> String {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    
    let mut hasher = DefaultHasher::new();
    keywords.join("-").hash(&mut hasher);
    format!("img_{:x}", hasher.finish())
}
