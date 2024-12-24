use std::sync::Arc;
use tokio::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use std::collections::HashMap;
use tokio::sync::Mutex;
use rig::{Error as RigError, Client as RigClient};

// Constants for rate limiting
const MAX_REQUESTS_PER_DAY: u32 = 3;
const RATE_LIMIT_HOURS: u64 = 24;

// Structure for Twitter mention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterMention {
    pub tweet_id: String,
    pub user_id: String,
    pub username: String,
    pub avatar_url: String,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// Structure for rate limiting
#[derive(Debug)]
struct RateLimit {
    count: u32,
    last_reset: Instant,
}

// Main Twitter handler
pub struct TwitterHandler {
    rig_client: Arc<RigClient>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimit>>>,
}

impl TwitterHandler {
    // Initialize Twitter handler
    pub fn new(rig_client: RigClient) -> Self {
        TwitterHandler {
            rig_client: Arc::new(rig_client),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Listen for mentions
    pub async fn listen_mentions(&self) -> Result<Vec<TwitterMention>, TwitterError> {
        info!("Listening for Twitter mentions...");
        
        let mentions = self.rig_client.get_social_mentions()
            .await
            .map_err(TwitterError::ClientError)?;
            
        let typed_mentions = mentions.into_iter()
            .filter_map(|mention| self.convert_to_twitter_mention(mention))
            .collect::<Vec<_>>();
        
        let valid_mentions = self.filter_valid_mentions(typed_mentions).await?;
        
        Ok(valid_mentions)
    }

    // Convert Rig mention to TwitterMention
    fn convert_to_twitter_mention(&self, rig_mention: rig::SocialMention) -> Option<TwitterMention> {
        Some(TwitterMention {
            tweet_id: rig_mention.id?,
            user_id: rig_mention.user_id?,
            username: rig_mention.username?,
            avatar_url: rig_mention.avatar_url?,
            text: rig_mention.text?,
            timestamp: rig_mention.created_at?,
        })
    }

    // Check if mention is valid and within rate limits
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

    // Rate limit checking
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

    // Send reply to user
    pub async fn send_reply(
        &self,
        mention: &TwitterMention,
        image: Vec<u8>,
        story: String,
    ) -> Result<(), TwitterError> {
        info!("Sending reply to user: {}", mention.username);
        
        let media_id = self.rig_client.upload_media(&image)
            .await
            .map_err(TwitterError::ClientError)?;
        
        let reply_text = format!(
            "@{} Here's your cat illustration with a story:\n\n{}",
            mention.username,
            story
        );

        self.rig_client.social_reply(
            &mention.tweet_id,
            &reply_text,
            Some(&media_id)
        )
        .await
        .map_err(TwitterError::ClientError)?;

        info!("Reply sent successfully to: {}", mention.username);
        Ok(())
    }
}

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum TwitterError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid mention format")]
    InvalidMention,
    
    #[error("Client error: {0}")]
    ClientError(#[from] RigError),
}

// Implement conversion from TwitterError to RigError
impl From<TwitterError> for RigError {
    fn from(err: TwitterError) -> RigError {
        RigError::Custom(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rate_limit() {
        let client = RigClient::new().unwrap();
        let handler = TwitterHandler::new(client);
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
