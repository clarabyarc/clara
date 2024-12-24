use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use log::{info, warn};
use rig_core::prelude::*;
use rig_core::config::Config;
use rig_core::metrics::Metrics as RigMetrics;

const CACHE_TIMEOUT_SECS: u64 = 86400;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    data: String,
    timestamp: SystemTime,
}

pub struct CacheManager {
    cache: HashMap<String, CacheEntry>,
    config: Config,
}

impl CacheManager {
    pub fn new() -> Self {
        CacheManager {
            cache: HashMap::new(),
            config: Config::default(),
        }
    }

    pub fn set(&mut self, key: &str, data: String) -> Result<(), UtilError> {
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now(),
        };
        
        self.cache.insert(key.to_string(), entry);
        self.cleanup();
        Ok(())
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
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

    pub fn exists(&self, key: &str) -> bool {
        self.cache.contains_key(key)
    }

    fn is_entry_valid(&self, entry: &CacheEntry) -> bool {
        entry.timestamp
            .elapsed()
            .map(|elapsed| elapsed < Duration::from_secs(CACHE_TIMEOUT_SECS))
            .unwrap_or(false)
    }

    fn cleanup(&mut self) {
        self.cache.retain(|_, entry| self.is_entry_valid(entry));
    }
}

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

impl From<UtilError> for rig_core::Error {
    fn from(err: UtilError) -> Self {
        rig_core::Error::Provider(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub twitter: TwitterConfig,
    pub vision: VisionConfig,
    pub image: ImageConfig,
    pub story: StoryConfig,
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
pub struct ImageConfig {
    pub size: String,
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryConfig {
    pub max_tokens: usize,
    pub temperature: f32,
}

impl AppConfig {
    pub async fn load() -> Result<Self, UtilError> {
        let config = Config::from_env()
            .map_err(|e| UtilError::ConfigError(e.to_string()))?;
            
        Ok(AppConfig::from(config))
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            twitter: TwitterConfig {
                rate_limit_window: 3600,
                max_calls_per_window: 100,
            },
            vision: VisionConfig {
                confidence_threshold: 0.75,
                max_labels: 4,
            },
            image: ImageConfig {
                size: String::from("512x512"),
                style: String::from("children's book illustration"),
            },
            story: StoryConfig {
                max_tokens: 280,
                temperature: 0.7,
            },
        }
    }
}

pub struct AppMetrics {
    metrics: RigMetrics,
    requests: usize,
    successes: usize,
    failures: usize,
    average_response_time: f64,
}

impl AppMetrics {
    pub fn new(metrics: RigMetrics) -> Self {
        AppMetrics {
            metrics,
            requests: 0,
            successes: 0,
            failures: 0,
            average_response_time: 0.0,
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
        
        // Record metrics using rig's metrics
        self.metrics.record_request(success, response_time);
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
        let data = "test_data".to_string();
        
        assert!(cache.set(key, data.clone()).is_ok());
        assert_eq!(cache.get(key), Some(data));
        assert!(cache.exists(key));
    }
}
