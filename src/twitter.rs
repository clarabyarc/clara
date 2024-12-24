use std::sync::Arc;
use tokio::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, warn};
use std::collections::HashMap;
use tokio::sync::Mutex;
use rig::providers::openai::Client;
use async_trait::async_trait;

const MAX_REQUESTS_PER_DAY: u32 = 3;
const RATE_LIMIT_HOURS: u64 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterMention {
    pub tweet_id: String,
    pub user_id: String,
    pub username: String,
    pub avatar_url: String,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
struct RateLimit {
    count: u32,
    last_reset: Instant,
}

#[async_trait]
pub trait SocialMediaClient: Send + Sync {
    async fn get_mentions(&self) -> Result<Vec<TwitterMention>, TwitterError>;
    async fn upload_media(&self, media: Vec<u8>, media_type: &str) -> Result<String, TwitterError>;
    async fn send_reply(&self, tweet_id: &str, text: &str, media_id: Option<&str>) -> Result<(), TwitterError>;
}

pub struct TwitterHandler {
    client: Client,
    social_client: Arc<Box<dyn SocialMediaClient>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimit>>>,
}

impl TwitterHandler {
    pub fn new(openai_client: &Client) -> Self {
        let social_client = Arc::new(Box::new(TwitterSocialClient::new()) as Box<dyn SocialMediaClient>);
        
        TwitterHandler {
            client: openai_client.clone(),
            social_client,
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn listen_mentions(&self) -> Result<Vec<TwitterMention>, TwitterError> {
        info!("Listening for Twitter mentions...");
        
        let mentions = self.social_client
            .get_mentions()
            .await?;
            
        let valid_mentions = self.filter_valid_mentions(mentions).await?;
        
        Ok(valid_mentions)
    }

    async fn filter_valid_mentions(
        &self,
        mentions: Vec<TwitterMention>
    ) -> Result<Vec<TwitterMention>, TwitterError> {
        let mut valid_mentions = Vec::new();
        
        for mention in mentions {
            if !mention.text.to_lowercase().contains("draw for my avatar") {
                continue;
            }

            if self.check_rate_limit(&mention.user_id).await? {
                valid_mentions.push(mention);
            }
        }

        Ok(valid_mentions)
    }

    async fn check_rate_limit(&self, user_id: &str) -> Result<bool, TwitterError> {
        let mut rate_limits = self.rate_limits.lock().await;
        
        let now = Instant::now();
        let rate_limit = rate_limits.entry(user_id.to_string())
            .or_insert_with(|| RateLimit {
                count: 0,
                last_reset: now,
            });

        if now.duration_since(rate_limit.last_reset) >= Duration::from_secs(RATE_LIMIT_HOURS * 3600) {
            rate_limit.count = 0;
            rate_limit.last_reset = now;
        }

        if rate_limit.count >= MAX_REQUESTS_PER_DAY {
            warn!("Rate limit exceeded for user: {}", user_id);
            return Ok(false);
        }

        rate_limit.count += 1;
        Ok(true)
    }

    pub async fn send_reply(
        &self,
        mention: &TwitterMention,
        image: Vec<u8>,
        story: String,
    ) -> Result<(), TwitterError> {
        info!("Sending reply to user: {}", mention.username);
        
        let media_id = self.social_client
            .upload_media(image, "image/jpeg")
            .await?;
        
        let reply_text = format!(
            "@{} Here's your cat illustration with a story:\n\n{}",
            mention.username,
            story
        );

        self.social_client
            .send_reply(&mention.tweet_id, &reply_text, Some(&media_id))
            .await?;

        info!("Reply sent successfully to: {}", mention.username);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TwitterError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid mention format")]
    InvalidMention,
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Client error: {0}")]
    ClientError(String),
}

// Mock implementation for testing
struct TwitterSocialClient {}

impl TwitterSocialClient {
    fn new() -> Self {
        TwitterSocialClient {}
    }
}

#[async_trait]
impl SocialMediaClient for TwitterSocialClient {
    async fn get_mentions(&self) -> Result<Vec<TwitterMention>, TwitterError> {
        // Implementation would go here
        Ok(Vec::new())
    }

    async fn upload_media(&self, _media: Vec<u8>, _media_type: &str) -> Result<String, TwitterError> {
        // Implementation would go here
        Ok("media_id".to_string())
    }

    async fn send_reply(&self, _tweet_id: &str, _text: &str, _media_id: Option<&str>) -> Result<(), TwitterError> {
        // Implementation would go here
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit() {
        let openai_client = Client::from_env().unwrap();
        let handler = TwitterHandler::new(&openai_client);
        let user_id = "test_user";

        // First request should succeed
        assert!(handler.check_rate_limit(user_id).await.unwrap());

        // Multiple requests up to limit should succeed
        for _ in 1..MAX_REQUESTS_PER_DAY {
            assert!(handler.check_rate_limit(user_id).await.unwrap());
        }

        // Request after limit should fail
        assert!(!handler.check_rate_limit(user_id).await.unwrap());
    }
}
