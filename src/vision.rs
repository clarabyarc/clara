use serde::{Deserialize, Serialize};
use log::{info, error};
use rig::prelude::*;
use rig::providers::google::vision::{Client as VisionClient, ImageRequest, Feature, FeatureType};

const MAX_LABELS: usize = 4;
const MIN_CONFIDENCE: f32 = 0.75;

#[derive(Debug, Deserialize)]
struct LabelAnnotation {
    description: String,
    score: f32,
}

pub struct VisionHandler {
    client: VisionClient,
    config: VisionConfig,
}

impl VisionHandler {
    pub fn new(openai_client: &rig::providers::openai::Client) -> Result<Self, VisionError> {
        let client = VisionClient::from_env()
            .map_err(|e| VisionError::InitializationError(e.to_string()))?;

        Ok(VisionHandler {
            client,
            config: VisionConfig::default(),
        })
    }

    pub async fn analyze_image(&self, image_url: &str) -> Result<Vec<String>, VisionError> {
        info!("Analyzing image: {}", image_url);

        let request = self.build_request(image_url);
        let response = self.send_request(request).await?;
        let keywords = self.process_response(response)?;

        info!("Image analysis completed. Keywords: {:?}", keywords);
        Ok(keywords)
    }

    fn build_request(&self, image_url: &str) -> ImageRequest {
        ImageRequest::new()
            .image_uri(image_url)
            .add_feature(Feature::new(
                FeatureType::LabelDetection,
                self.config.max_labels as i32
            ))
    }

    async fn send_request(&self, request: ImageRequest) -> Result<Vec<LabelAnnotation>, VisionError> {
        let response = self.client
            .analyze_image(request)
            .await
            .map_err(|e| VisionError::ApiError(e.to_string()))?;

        Ok(response.label_annotations)
    }

    fn process_response(&self, annotations: Vec<LabelAnnotation>) -> Result<Vec<String>, VisionError> {
        let mut keywords: Vec<String> = annotations
            .into_iter()
            .filter(|label| label.score >= self.config.confidence_threshold)
            .take(self.config.max_labels)
            .map(|label| label.description.to_lowercase())
            .collect();

        if keywords.is_empty() {
            keywords.push("person".to_string());
        }

        Ok(keywords)
    }

    pub async fn validate_image_url(&self, url: &str) -> Result<bool, VisionError> {
        self.client
            .validate_image_url(url)
            .await
            .map_err(|_| VisionError::InvalidImageUrl)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("Failed to analyze image: {0}")]
    AnalysisFailed(String),
    
    #[error("Invalid image URL")]
    InvalidImageUrl,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Initialization error: {0}")]
    InitializationError(String),
}

impl From<VisionError> for rig::Error {
    fn from(err: VisionError) -> Self {
        rig::Error::Provider(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    pub max_labels: usize,
    pub confidence_threshold: f32,
}

impl Default for VisionConfig {
    fn default() -> Self {
        VisionConfig {
            max_labels: MAX_LABELS,
            confidence_threshold: MIN_CONFIDENCE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::providers::openai::Client as OpenAIClient;

    async fn setup_test_handler() -> VisionHandler {
        let openai_client = OpenAIClient::from_env().unwrap();
        VisionHandler::new(&openai_client).unwrap()
    }

    #[tokio::test]
    async fn test_validate_image_url() {
        let handler = setup_test_handler().await;
        
        // Test valid image URL
        assert!(handler.validate_image_url("https://example.com/image.jpg").await.unwrap());
        
        // Test invalid image URL
        assert!(!handler.validate_image_url("https://example.com/not-image").await.unwrap());
    }

    #[tokio::test]
    async fn test_analyze_image() {
        let handler = setup_test_handler().await;
        let image_url = "https://example.com/test.jpg";
        
        let keywords = handler.analyze_image(image_url).await.unwrap();
        assert!(!keywords.is_empty());
        assert!(keywords.len() <= MAX_LABELS);
    }
}
