use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};
use dotenv::dotenv;
use rig::{Error as RigError, Client as RigClient};

mod twitter;
mod vision;
mod image;
mod story;
mod utils;

use crate::twitter::TwitterHandler;
use crate::vision::VisionHandler;
use crate::image::ImageGenerator;
use crate::story::StoryGenerator;
use crate::utils::CacheManager;

// Main application structure
pub struct Clara {
    twitter_handler: Arc<TwitterHandler>,
    vision_handler: Arc<VisionHandler>,
    image_generator: Arc<ImageGenerator>,
    story_generator: Arc<StoryGenerator>,
    cache_manager: Arc<Mutex<CacheManager>>,
    max_concurrent_requests: usize,
}

impl Clara {
    // Initialize Clara instance
    pub async fn new() -> Result<Self, AppError> {
        // Load environment variables
        dotenv().ok();
        
        // Initialize logger
        env_logger::init();
        
        info!("Initializing Clara bot...");
        
        let rig_client = RigClient::new().map_err(|e| AppError::RigError(e))?;
        
        // Create handlers with default configuration
        let twitter_handler = Arc::new(TwitterHandler::new(rig_client));
        let vision_handler = Arc::new(VisionHandler::new()?);
        let image_generator = Arc::new(ImageGenerator::new()?);
        let story_generator = Arc::new(StoryGenerator::new()?);
        let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
        
        Ok(Clara {
            twitter_handler,
            vision_handler,
            image_generator,
            story_generator,
            cache_manager,
            max_concurrent_requests: 10,
        })
    }

    // Start the main service loop
    pub async fn start(&self) -> Result<(), AppError> {
        info!("Clara bot is starting...");
        
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent_requests));
        
        loop {
            match self.twitter_handler.listen_mentions().await {
                Ok(mentions) => {
                    for mention in mentions {
                        let sem_clone = semaphore.clone();
                        let twitter_handler = self.twitter_handler.clone();
                        let vision_handler = self.vision_handler.clone();
                        let image_generator = self.image_generator.clone();
                        let story_generator = self.story_generator.clone();
                        let cache_manager = self.cache_manager.clone();
                        
                        tokio::spawn(async move {
                            let _permit = sem_clone.acquire().await.unwrap();
                            if let Err(e) = Self::handle_mention(
                                mention,
                                twitter_handler,
                                vision_handler,
                                image_generator,
                                story_generator,
                                cache_manager
                            ).await {
                                error!("Error processing mention: {}", e);
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("Error while listening for mentions: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    // Handle individual mention
    async fn handle_mention(
        mention: twitter::TwitterMention,
        twitter_handler: Arc<TwitterHandler>,
        vision_handler: Arc<VisionHandler>,
        image_generator: Arc<ImageGenerator>,
        story_generator: Arc<StoryGenerator>,
        cache_manager: Arc<Mutex<CacheManager>>,
    ) -> Result<(), AppError> {
        info!("Processing mention from @{}", mention.username);

        let cache_key = format!("mention_{}", mention.tweet_id);
        if cache_manager.lock().await.exists(&cache_key) {
            info!("Found cached response for mention");
            return Ok(());
        }

        let keywords = vision_handler.analyze_image(&mention.avatar_url).await?;
        let image_data = image_generator.generate_cat_image(&keywords).await?;
        let story = story_generator.generate_story(&keywords).await?;
        
        twitter_handler.send_reply(&mention, image_data, story).await?;
        
        cache_manager.lock().await.set(&cache_key, "completed".as_bytes().to_vec())?;
        
        info!("Successfully processed mention from @{}", mention.username);
        Ok(())
    }
}

// Custom error type for application
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Rig error: {0}")]
    RigError(#[from] RigError),

    #[error("Twitter error: {0}")]
    TwitterError(#[from] twitter::TwitterError),
    
    #[error("Vision error: {0}")]
    VisionError(#[from] vision::VisionError),
    
    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),
    
    #[error("Story error: {0}")]
    StoryError(#[from] story::StoryError),

    #[error("Cache error: {0}")]
    CacheError(#[from] utils::UtilError),
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let clara = Clara::new().await?;
    clara.start().await?;
    
    Ok(())
}
