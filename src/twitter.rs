use std::error::Error;
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use std::collections::HashMap;
use tokio::sync::Mutex;

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
    rig_client: Arc<rig::RigClient>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimit>>>,
}

impl TwitterHandler {
    // Initialize Twitter handler
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let rig_client = Arc::new(rig::RigClient::new()?);
        
        Ok(TwitterHandler {
            rig_client,
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // Listen for mentions
    pub async fn listen_mentions(&self) -> Result<Vec<TwitterMention>, Box<dyn Error>> {
        info!("Listening for Twitter mentions...");
        
        // Get mentions through RIG client
        let mentions = self.rig_client.get_mentions().await?;
        
        // Filter valid mentions
        let valid_mentions = self.filter_valid_mentions(mentions).await?;
        
        Ok(valid_mentions)
    }

    // Check if mention is valid and within rate limits
    async fn filter_valid_mentions(
        &self,
        mentions: Vec<TwitterMention>
    ) -> Result<Vec<TwitterMention>, Box<dyn Error>> {
        let mut valid_mentions = Vec::new();
        
        for mention in mentions {
            // Check if mention contains the trigger phrase
            if !mention.text.to_lowercase().contains("draw for my avatar") {
                continue;
            }

            // Check rate limit
            if self.check_rate_limit(&mention.user_id).await? {
                valid_mentions.push(mention);
            }
        }

        Ok(valid_mentions)
    }

    // Rate limit checking
    async fn check_rate_limit(&self, user_id: &str) -> Result<bool, Box<dyn Error>> {
        let mut rate_limits = self.rate_limits.lock().await;
        
        let now = Instant::now();
        let rate_limit = rate_limits.entry(user_id.to_string())
            .or_insert_with(|| RateLimit {
                count: 0,
                last_reset: now,
            });

        // Reset counter if 24 hours have passed
        if now.duration_since(rate_limit.last_reset) > Duration::from_hours(RATE_LIMIT_HOURS) {
            rate_limit.count = 0;
            rate_limit.last_reset = now;
        }

        // Check if user has exceeded rate limit
        if rate_limit.count >= MAX_REQUESTS_PER_DAY {
            warn!("Rate limit exceeded for user: {}", user_id);
            return Ok(false);
        }

        // Increment counter
        rate_limit.count += 1;
        Ok(true)
    }

    // Send reply to user
    pub async fn send_reply(
        &self,
        mention: &TwitterMention,
        image: Vec<u8>,
        story: String,
    ) -> Result<(), Box<dyn Error>> {
        info!("Sending reply to user: {}", mention.username);
        
        // Upload image and get media ID
        let media_id = self.rig_client.upload_media(&image).await?;
        
        // Construct reply text
        let reply_text = format!(
            "@{} Here's your cat illustration with a story:\n\n{}",
            mention.username,
            story
        );

        // Send tweet with media
        self.rig_client.reply_with_media(
            &mention.tweet_id,
            &reply_text,
            &media_id
        ).await?;

        info!("Reply sent successfully to: {}", mention.username);
        Ok(())
    }
}

// Error handling
#[derive(Debug, thiserror::Error)]
pub enum TwitterError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Invalid mention format")]
    InvalidMention,
}
