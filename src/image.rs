use std::error::Error;
use serde::{Deserialize, Serialize};
use log::{info, error};
use reqwest::Client;
use tokio::time::Duration;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

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
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("DALLE_API_KEY")
            .map_err(|_| "Missing DALLE_API_KEY environment variable")?;
            
        let client = Client::builder()
            .timeout(Duration::from_secs(DALLE_API_TIMEOUT))
            .build()?;

        Ok(ImageGenerator {
            client,
            api_key,
            api_endpoint: "https://api.openai.com/v1/images/generations".to_string(),
        })
    }

    // Generate image based on keywords
    pub async fn generate_cat_image(&self, keywords: &[String]) -> Result<Vec<u8>, Box<dyn Error>> {
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
            prompt: prompt.to_string(),
            n: 1,
            size: IMAGE_SIZE.to_string(),
            response_format: "b64_json".to_string(),
        }
    }

    // Send request to DALL-E API
    async fn send_request(&self, request: DallERequest) -> Result<DallEResponse, Box<dyn Error>> {
        let response = self.client
            .post(&self.api_endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("DALL-E API error: {}", error_text);
            return Err(ImageError::ApiError(error_text).into());
        }

        let dalle_response = response.json::<DallEResponse>().await?;
        Ok(dalle_response)
    }

    // Process DALL-E API response
    fn process_response(&self, response: DallEResponse) -> Result<Vec<u8>, Box<dyn Error>> {
        let image_data = response.data
            .first()
            .ok_or(ImageError::NoImageGenerated)?;

        // Decode base64 image data
        let image_bytes = BASE64.decode(&image_data.b64_json)?;

        Ok(image_bytes)
    }

    // Apply post-processing to generated image
    pub fn post_process_image(&self, image_data: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
        // For now, just return the original image
        // TODO: Implement image post-processing if needed
        Ok(image_data)
    }

    // Validate generated image
    pub fn validate_image(&self, image_data: &[u8]) -> Result<bool, Box<dyn Error>> {
        // Check if image data is valid
        if image_data.is_empty() {
            return Ok(false);
        }

        // Check if image data starts with JPEG header
        if image_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Ok(true);
        }

        Err(ImageError::InvalidImageFormat.into())
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
    
    #[error("Image generation failed: {0}")]
    GenerationFailed(String),
}

// Image generation configuration
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub size: String,
    pub style: String,
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig {
            size: IMAGE_SIZE.to_string(),
            style: DEFAULT_STYLE.to_string(),
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
