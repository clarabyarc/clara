use std::error::Error;
use serde::{Deserialize, Serialize};
use log::{info, error};
use tokio::time::Duration;
use reqwest::Client;

// Constants for Vision API
const VISION_API_TIMEOUT: u64 = 10;
const MAX_LABELS: usize = 4;
const MIN_CONFIDENCE: f32 = 0.75;

// Structure for Vision API response
#[derive(Debug, Deserialize)]
struct VisionApiResponse {
    responses: Vec<AnnotateImageResponse>,
}

#[derive(Debug, Deserialize)]
struct AnnotateImageResponse {
    #[serde(default)]
    label_annotations: Vec<LabelAnnotation>,
    #[serde(default)]
    error: Option<ApiVisionError>,
}

#[derive(Debug, Deserialize)]
struct LabelAnnotation {
    description: String,
    score: f32,
}

// Renamed to avoid conflict with the main error enum
#[derive(Debug, Deserialize)]
struct ApiVisionError {
    code: i32,
    message: String,
}

// Request structures
#[derive(Debug, Serialize)]
struct VisionApiRequest {
    requests: Vec<AnnotateImageRequest>,
}

#[derive(Debug, Serialize)]
struct AnnotateImageRequest {
    image: ImageSource,
    features: Vec<FeatureType>,
}

#[derive(Debug, Serialize)]
struct ImageSource {
    #[serde(rename = "imageUri")]
    image_uri: String,
}

#[derive(Debug, Serialize)]
struct FeatureType {
    #[serde(rename = "type")]
    feature_type: String,
    #[serde(rename = "maxResults")]
    max_results: i32,
}

// Main Vision handler
pub struct VisionHandler {
    client: Client,
    api_key: String,
    api_endpoint: String,
}

impl VisionHandler {
    // Initialize Vision handler
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let api_key = std::env::var("VISION_API_KEY")
            .map_err(|_| "Missing VISION_API_KEY environment variable")?;
            
        let client = Client::builder()
            .timeout(Duration::from_secs(VISION_API_TIMEOUT))
            .build()?;

        Ok(VisionHandler {
            client,
            api_key,
            api_endpoint: "https://vision.googleapis.com/v1/images:annotate".to_string(),
        })
    }

    // Analyze image from URL
    pub async fn analyze_image(&self, image_url: &str) -> Result<Vec<String>, VisionError> {
        info!("Analyzing image: {}", image_url);

        let request = self.build_request(image_url);
        let response = self.send_request(request).await
            .map_err(|e| VisionError::AnalysisFailed(e.to_string()))?;
        let keywords = self.process_response(response)
            .map_err(|e| VisionError::AnalysisFailed(e.to_string()))?;

        info!("Image analysis completed. Keywords: {:?}", keywords);
        Ok(keywords)
    }

    // Build Vision API request
    fn build_request(&self, image_url: &str) -> VisionApiRequest {
        VisionApiRequest {
            requests: vec![AnnotateImageRequest {
                image: ImageSource {
                    image_uri: image_url.to_string(),
                },
                features: vec![FeatureType {
                    feature_type: "LABEL_DETECTION".to_string(),
                    max_results: MAX_LABELS as i32,
                }],
            }],
        }
    }

    // Send request to Vision API
    async fn send_request(&self, request: VisionApiRequest) -> Result<VisionApiResponse, Box<dyn Error>> {
        let response = self.client
            .post(&self.api_endpoint)
            .query(&[("key", &self.api_key)])
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Vision API error: {}", error_text);
            return Err(VisionError::ApiError(error_text).into());
        }

        let vision_response = response.json::<VisionApiResponse>().await?;
        Ok(vision_response)
    }

    // Process Vision API response
    fn process_response(&self, response: VisionApiResponse) -> Result<Vec<String>, Box<dyn Error>> {
        let annotations = response.responses
            .first()
            .ok_or("Empty response from Vision API")?;

        if let Some(error) = &annotations.error {
            return Err(VisionError::ApiError(error.message.clone()).into());
        }

        // Filter and transform labels
        let mut keywords: Vec<String> = annotations.label_annotations
            .iter()
            .filter(|label| label.score >= MIN_CONFIDENCE)
            .take(MAX_LABELS)
            .map(|label| label.description.to_lowercase())
            .collect();

        // Ensure we have at least some keywords
        if keywords.is_empty() {
            keywords.push("person".to_string());
        }

        Ok(keywords)
    }

    // Validate image URL
    pub async fn validate_image_url(&self, url: &str) -> Result<bool, VisionError> {
        let response = self.client
            .head(url)
            .send()
            .await
            .map_err(|e| VisionError::InvalidImageUrl)?;

        // Check if the URL points to a valid image
        if let Some(content_type) = response.headers().get("content-type") {
            let content_type = content_type.to_str()
                .map_err(|_| VisionError::InvalidImageUrl)?;
            Ok(content_type.starts_with("image/"))
        } else {
            Ok(false)
        }
    }
}

// Custom error type for Vision operations
#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("Failed to analyze image: {0}")]
    AnalysisFailed(String),
    #[error("Invalid image URL")]
    InvalidImageUrl,
    #[error("API error: {0}")]
    ApiError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
