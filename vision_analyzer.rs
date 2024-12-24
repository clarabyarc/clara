use serde::{Deserialize, Serialize};
use log::{info, error};
use rig::providers::openai::Client;
use rig::completion::{Completion, Message}; 
use async_trait::async_trait;
use anyhow::Result;

const MAX_LABELS: usize = 4;
const MIN_CONFIDENCE: f32 = 0.75;

#[derive(Debug, Deserialize)]
struct LabelAnnotation {
    description: String,
    score: f32,
}

#[async_trait]
trait VisionService {
    async fn analyze_image(&self, image_url: &str) -> Result<Vec<LabelAnnotation>, VisionError>;
    async fn validate_image_url(&self, url: &str) -> Result<bool, VisionError>;
}

pub struct VisionAnalyzer {
    client: Client,
    config: VisionConfig,
}

impl VisionAnalyzer {
    pub fn new(openai_client: &Client) -> Result<Self, VisionError> {
        Ok(VisionAnalyzer {
            client: openai_client.clone(),
            config: VisionConfig::default(),
        })
    }

    pub async fn analyze_image(&self, image_url: &str) -> Result<Vec<String>, VisionError> {
        info!("Analyzing image: {}", image_url);

        let agent = self.client
            .agent("gpt-4-vision-preview")
            .build();
        
        let prompt = format!(
            "You are a vision analysis assistant. Analyze images and provide labels with confidence scores.\n\n\
            Analyze this image {} and provide up to {} labels with confidence above {}. \
            Format each label as 'label:confidence'. \
            Focus on clear, descriptive labels.",
            image_url,
            self.config.max_labels,
            self.config.confidence_threshold
        );

        let messages = vec![Message {
            role: "user".to_string(),
            content: prompt.clone(),
        }];

        let response = agent
            .completion(&messages[0].content, messages)
            .await
            .map_err(|e| VisionError::ApiError(e.to_string()))?;

        let keywords = self.process_response(&response.text)?;

        info!("Image analysis completed. Keywords: {:?}", keywords);
        Ok(keywords)
    }

    fn process_response(&self, response: &str) -> Result<Vec<String>, VisionError> {
        let mut keywords = Vec::new();
        
        for line in response.lines() {
            if let Some((label, confidence)) = line.split_once(':') {
                if let Ok(score) = confidence.trim().parse::<f32>() {
                    if score >= self.config.confidence_threshold {
                        keywords.push(label.trim().to_lowercase());
                    }
                }
            }
        }

        // Fallback for empty results
        if keywords.is_empty() {
            keywords.push("unclassified".to_string());
        }

        Ok(keywords.into_iter().take(self.config.max_labels).collect())
    }

    pub async fn validate_image_url(&self, url: &str) -> Result<bool, VisionError> {
        if !url.starts_with("http") || !url.contains('.') {
            return Ok(false);
        }

        let client = reqwest::Client::new();
        let response = client
            .head(url)
            .send()
            .await
            .map_err(|_| VisionError::InvalidImageUrl)?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        Ok(content_type.starts_with("image/"))
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
    use tokio;

    async fn setup_test_analyzer() -> VisionAnalyzer {
        let openai_client = Client::from_env().unwrap();
        VisionAnalyzer::new(&openai_client).unwrap()
    }

    #[tokio::test]
    async fn test_validate_image_url() {
        let analyzer = setup_test_analyzer().await;
        
        // Test valid image URL
        let valid_url = "https://example.com/image.jpg";
        assert!(analyzer.validate_image_url(valid_url).await.unwrap());
        
        // Test invalid image URL
        let invalid_url = "not-a-url";
        assert!(!analyzer.validate_image_url(invalid_url).await.unwrap());
    }

    #[tokio::test]
    async fn test_analyze_image() {
        let analyzer = setup_test_analyzer().await;
        let image_url = "https://example.com/test.jpg";
        
        let keywords = analyzer.analyze_image(image_url).await.unwrap();
        assert!(!keywords.is_empty());
        assert!(keywords.len() <= MAX_LABELS);
    }
}
