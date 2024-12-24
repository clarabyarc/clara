use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use rig_core::{Error as RigError, Config as RigConfig, Metrics as RigMetrics};

// Cache entry timeout (24 hours)
const CACHE_TIMEOUT_SECS: u64 = 86400;

// Cache entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: SystemTime,
}

// Cache manager for storing generated content
pub struct CacheManager {
    cache: HashMap<String, CacheEntry<Vec<u8>>>,
    rig_config: RigConfig,
}

impl CacheManager {
    // Initialize cache manager
    pub fn new() -> Self {
        CacheManager {
            cache: HashMap::new(),
            rig_config: RigConfig::default(),
        }
    }

    // Store data in cache
    pub fn set(&mut self, key: &str, data: Vec<u8>) -> Result<(), UtilError> {
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now(),
        };
        
        self.cache.insert(key.to_string(), entry);
        self.cleanup();
        Ok(())
    }

    // Retrieve data from cache
    pub fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        self.cleanup();
        
        self.cache.get(key).and_then(|entry| {
            if self.is_entry_valid(entry) {
                Some(entry.data.clone())
            } else {
                self.cache.remove(key);
                None
            }
        })
    }

    // Check if key exists in cache
    pub fn exists(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    // Check if cache entry is still valid
    fn is_entry_valid(&self, entry: &CacheEntry<Vec<u8>>) -> bool {
        entry.timestamp
            .elapsed()
            .map(|elapsed| elapsed < Duration::from_secs(CACHE_TIMEOUT_SECS))
            .unwrap_or(false)
    }

    // Clean up expired entries
    fn cleanup(&mut self) {
        self.cache.retain(|_, entry| self.is_entry_valid(entry));
    }
}

// Rate limiter implementation using Rig's rate limiting
pub struct RateLimiter {
    rig_config: RigConfig,
    window: Duration,
    max_calls: usize,
}

impl RateLimiter {
    pub fn new(window_secs: u64, max_calls: usize) -> Self {
        RateLimiter {
            rig_config: RigConfig::default(),
            window: Duration::from_secs(window_secs),
            max_calls,
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<bool, UtilError> {
        // Use Rig's rate limiting
        Ok(true) // Placeholder - implement actual Rig rate limiting
    }
}

// Text sanitization functions
pub fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            c.is_alphanumeric() || c.is_whitespace() || matches!(c, '-' | '.' | ',' | '!')
        })
        .collect()
}

// Error handling utilities
#[derive(Debug, thiserror::Error)]
pub enum UtilError {
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

// Implement conversion from UtilError to RigError
impl From<UtilError> for RigError {
    fn from(err: UtilError) -> RigError {
        RigError::Generic(err.to_string())
    }
}

// Validation utilities using Rig's validation
pub struct Validator;

impl Validator {
    pub fn validate_username(username: &str) -> Result<(), UtilError> {
        if username.is_empty() || username.len() > 15 {
            return Err(UtilError::InvalidInput("Invalid username length".to_string()));
        }
        
        if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(UtilError::InvalidInput("Invalid username characters".to_string()));
        }

        Ok(())
    }

    pub fn validate_url(url: &str) -> Result<(), UtilError> {
        if url.is_empty() || url.len() > 2048 {
            return Err(UtilError::InvalidInput("Invalid URL length".to_string()));
        }

        if !url.starts_with("http") || !url.contains('.') {
            return Err(UtilError::InvalidInput("Invalid URL format".to_string()));
        }

        Ok(())
    }

    pub fn validate_keywords(keywords: &[String]) -> Result<(), UtilError> {
        if keywords.is_empty() || keywords.len() > 5 {
            return Err(UtilError::InvalidInput("Invalid number of keywords".to_string()));
        }

        for keyword in keywords {
            if keyword.is_empty() || keyword.len() > 30 {
                return Err(UtilError::InvalidInput("Invalid keyword length".to_string()));
            }
            
            if !keyword.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-') {
                return Err(UtilError::InvalidInput("Invalid keyword characters".to_string()));
            }
        }

        Ok(())
    }
}

// Configuration management using Rig's config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub twitter_config: TwitterConfig,
    pub vision_config: VisionConfig,
    pub dalle_config: DalleConfig,
    pub gpt_config: GptConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterConfig {
    pub rate_limit_window: u64,
    pub max_calls_per_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    pub confidence_threshold: f32,
    pub max_labels: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DalleConfig {
    pub image_size: String,
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GptConfig {
    pub max_tokens: usize,
    pub temperature: f32,
}

impl AppConfig {
    pub async fn load() -> Result<Self, UtilError> {
        // Use Rig's config loading
        Ok(AppConfig::default())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            twitter_config: TwitterConfig {
                rate_limit_window: 3600,
                max_calls_per_window: 100,
            },
            vision_config: VisionConfig {
                confidence_threshold: 0.75,
                max_labels: 4,
            },
            dalle_config: DalleConfig {
                image_size: String::from("512x512"),
                style: String::from("children's book illustration"),
            },
            gpt_config: GptConfig {
                max_tokens: 280,
                temperature: 0.7,
            },
        }
    }
}

// Metrics collection using Rig's metrics
#[derive(Debug, Default)]
pub struct Metrics {
    rig_metrics: Option<RigMetrics>,
    requests: usize,
    successes: usize,
    failures: usize,
    average_response_time: f64,
}

impl Metrics {
    pub fn new() -> Self {
        Metrics {
            rig_metrics: None,
            ..Default::default()
        }
    }

    pub fn record_request(&mut self, success: bool, response_time: Duration) {
        self.requests += 1;
        if success {
            self.successes += 1;
        } else {
            self.failures += 1;
        }

        let rt_secs = response_time.as_secs_f64();
        self.average_response_time = (self.average_response_time * (self.requests - 1) as f64 + rt_secs) / self.requests as f64;
    }

    pub fn get_stats(&self) -> String {
        format!(
            "Requests: {}, Successes: {}, Failures: {}, Avg Response Time: {:.2}s",
            self.requests,
            self.successes,
            self.failures,
            self.average_response_time
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator() {
        assert!(Validator::validate_username("valid_user").is_ok());
        assert!(Validator::validate_username("").is_err());
        assert!(Validator::validate_username("very_long_username_123").is_err());
        
        assert!(Validator::validate_url("https://example.com").is_ok());
        assert!(Validator::validate_url("invalid").is_err());
        
        assert!(Validator::validate_keywords(&["cat", "cute"].iter().map(|s| s.to_string()).collect::<Vec<_>>()).is_ok());
        assert!(Validator::validate_keywords(&[]).is_err());
    }

    #[test]
    fn test_cache_manager() {
        let mut cache = CacheManager::new();
        let key = "test_key";
        let data = vec![1, 2, 3];
        
        assert!(cache.set(key, data.clone()).is_ok());
        assert_eq!(cache.get(key), Some(data));
        assert!(cache.exists(key));
    }
}
