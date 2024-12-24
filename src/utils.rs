use std::collections::HashMap;
use std::error::Error;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use tokio::sync::Mutex;

// Cache entry timeout (24 hours)
const CACHE_TIMEOUT_SECS: u64 = 86400;

// Cache entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    data: Vec<u8>,
    timestamp: SystemTime,
}

// Cache manager for storing generated content
pub struct CacheManager {
    cache: HashMap<String, CacheEntry>,
}

impl CacheManager {
    // Initialize cache manager
    pub fn new() -> Self {
        CacheManager {
            cache: HashMap::new(),
        }
    }

    // Store data in cache
    pub fn set(&mut self, key: String, data: Vec<u8>) {
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now(),
        };
        
        self.cache.insert(key, entry);
        self.cleanup();
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

    // Check if cache entry is still valid
    fn is_entry_valid(&self, entry: &CacheEntry) -> bool {
        match entry.timestamp.elapsed() {
            Ok(elapsed) => elapsed < Duration::from_secs(CACHE_TIMEOUT_SECS),
            Err(_) => false,
        }
    }

    // Clean up expired entries
    fn cleanup(&mut self) {
        self.cache.retain(|_, entry| self.is_entry_valid(entry));
    }
}

// Rate limiter for API calls
pub struct RateLimiter {
    calls: HashMap<String, Vec<SystemTime>>,
    window: Duration,
    max_calls: usize,
}

impl RateLimiter {
    pub fn new(window_secs: u64, max_calls: usize) -> Self {
        RateLimiter {
            calls: HashMap::new(),
            window: Duration::from_secs(window_secs),
            max_calls,
        }
    }

    pub fn check_rate_limit(&mut self, key: &str) -> bool {
        let now = SystemTime::now();
        let calls = self.calls.entry(key.to_string()).or_insert_with(Vec::new);
        
        // Remove old calls
        calls.retain(|&time| {
            match time.elapsed() {
                Ok(elapsed) => elapsed < self.window,
                Err(_) => false,
            }
        });

        // Check if under limit
        if calls.len() < self.max_calls {
            calls.push(now);
            true
        } else {
            false
        }
    }
}

// Text sanitization functions
pub fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter(|&c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '.' || c == ',' || c == '!')
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
}

// Validation utilities
pub struct Validator;

impl Validator {
    // Validate Twitter username
    pub fn validate_username(username: &str) -> bool {
        if username.is_empty() || username.len() > 15 {
            return false;
        }
        
        username.chars().all(|c| {
            c.is_alphanumeric() || c == '_'
        })
    }

    // Validate image URL
    pub fn validate_url(url: &str) -> bool {
        if url.is_empty() || url.len() > 2048 {
            return false;
        }

        url.starts_with("http") && url.contains('.')
    }

    // Validate keywords
    pub fn validate_keywords(keywords: &[String]) -> bool {
        if keywords.is_empty() || keywords.len() > 5 {
            return false;
        }

        keywords.iter().all(|k| {
            !k.is_empty() && k.len() <= 30 && k.chars().all(|c| {
                c.is_alphanumeric() || c.is_whitespace() || c == '-'
            })
        })
    }
}

// Configuration management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
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

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        // Load from environment or config file
        // For now, return default config
        Ok(Config::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            twitter_config: TwitterConfig {
                rate_limit_window: 3600,
                max_calls_per_window: 100,
            },
            vision_config: VisionConfig {
                confidence_threshold: 0.75,
                max_labels: 4,
            },
            dalle_config: DalleConfig {
                image_size: "512x512".to_string(),
                style: "children's book illustration".to_string(),
            },
            gpt_config: GptConfig {
                max_tokens: 280,
                temperature: 0.7,
            },
        }
    }
}

// Metrics collection
#[derive(Debug, Default)]
pub struct Metrics {
    requests: usize,
    successes: usize,
    failures: usize,
    average_response_time: f64,
}

impl Metrics {
    pub fn new() -> Self {
        Metrics::default()
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
