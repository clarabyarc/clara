use std::sync::Arc;
use tokio::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, warn};
use std::collections::HashMap;
use tokio::sync::Mutex;
use rig::prelude::*;
use rig::providers::openai::Client as OpenAIClient;
use rig::social::{SocialClient, SocialMention, MediaUpload};

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

pub struct TwitterHandler {
    client: OpenAIClient,
    social_client: Arc<Box<dyn SocialClient>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimit>>>,
}

impl TwitterHandler {
    pub fn new(openai_client: &OpenAIClient) -> Self {
        // Initialize Twitter client using rig's social client
        let social_client = Arc::new(Box::new(TwitterSocialClient::new()) as Box<dyn SocialClient>);
        
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
            .await
            .map_err(TwitterError::ClientError)?;
            
        let typed_mentions = mentions.into_iter()
            .filter_map(|mention| self.convert_to_twitter_mention(mention))
            .collect::<Vec<_>>();
        
        let valid_mentions = self.filter_valid_mentions(typed_mentions).await?;
        
        Ok(valid_mentions)
    }

    fn convert_to_twitter_mention(&self, mention: SocialMention) -> Option<TwitterMention> {
        Some(TwitterMention {
            tweet_id: mention.id,
            user_id: mention.user_id,
            username: mention.username,
            avatar_url: mention.avatar_url,
            text: mention.text,
            timestamp: mention.created_at,
        })
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
        
        let media_upload = MediaUpload::new(image, "image/jpeg".to_string());
        let media_id = self.social_client
            .upload_media(media_upload)
            .await
            .map_err(TwitterError::ClientError)?;
        
        let reply_text = format!(
            "@{} Here's your cat illustration with a story:\n\n{}",
            mention.username,
            story
        );

        self.social_client
            .send_reply(&mention.tweet_id, &reply_text, Some(&media_id))
            .await
            .map_err(TwitterError::ClientError)?;

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
    
    #[error("Client error: {0}")]
    ClientError(rig::Error),
}

impl From<TwitterError> for rig::Error {
    fn from(err: TwitterError) -> Self {
        rig::Error::Provider(err.to_string())
    }
}

// Mock implementation for testing
struct TwitterSocialClient {}

impl TwitterSocialClient {
    fn new() -> Self {
        TwitterSocialClient {}
    }
}

#[async_trait::async_trait]
impl SocialClient for TwitterSocialClient {
    async fn get_mentions(&self) -> Result<Vec<SocialMention>, rig::Error> {
        // Implementation would go here
        Ok(Vec::new())
    }

    async fn upload_media(&self, _media: MediaUpload) -> Result<String, rig::Error> {
        // Implementation would go here
        Ok("media_id".to_string())
    }

    async fn send_reply(&self, _tweet_id: &str, _text: &str, _media_id: Option<&str>) -> Result<(), rig::Error> {
        // Implementation would go here
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit() {
        let openai_client = OpenAIClient::from_env().unwrap();
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
