use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use dotenv::dotenv;

mod twitter;
mod vision;
mod image;
mod story;
mod utils;

use crate::twitter::TwitterHandler;
use crate::utils::CacheManager;

// Main application structure
pub struct Clara {
    twitter_handler: Arc<TwitterHandler>,
    cache_manager: Arc<Mutex<CacheManager>>,
    max_concurrent_requests: usize,
}

impl Clara {
    // Initialize Clara instance
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        // Load environment variables
        dotenv().ok();
        
        // Initialize logger
        env_logger::init();
        
        info!("Initializing Clara bot...");
        
        // Create handlers with default configuration
        let twitter_handler = Arc::new(TwitterHandler::new()?);
        let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
        
        Ok(Clara {
            twitter_handler,
            cache_manager,
            max_concurrent_requests: 10,
        })
    }

    // Start the main service loop
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        info!("Clara bot is starting...");
        
        // Initialize request semaphore for concurrent request limiting
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_requests));
        
        loop {
            match self.twitter_handler.listen_mentions().await {
                Ok(mentions) => {
                    for mention in mentions {
                        let sem_clone = semaphore.clone();
                        let twitter_handler = self.twitter_handler.clone();
                        let cache_manager = self.cache_manager.clone();
                        
                        // Spawn a new task for each mention
                        tokio::spawn(async move {
                            let _permit = sem_clone.acquire().await.unwrap();
                            if let Err(e) = Self::handle_mention(
                                mention,
                                twitter_handler,
                                cache_manager
                            ).await {
                                error!("Error processing mention: {}", e);
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("Error while listening for mentions: {}", e);
                    // Add small delay before retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    // Handle individual mention
    async fn handle_mention(
        mention: twitter::TwitterMention,
        twitter_handler: Arc<TwitterHandler>,
        cache_manager: Arc<Mutex<CacheManager>>,
    ) -> Result<(), Box<dyn Error>> {
        // Implementation will be added as we develop other modules
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create and start Clara instance
    let clara = Clara::new().await?;
    clara.start().await?;
    
    Ok(())
}
