//! Authentication for single-user access

use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Authentication configuration
pub struct AuthConfig {
    pub enabled: bool,
    pub token: String,
    pub config_dir: PathBuf,
}

/// Authentication manager
pub struct AuthManager {
    config: AuthConfig,
}

impl AuthManager {
    /// Create a new auth manager
    pub fn new(config: AuthConfig) -> Self {
        fs::create_dir_all(&config.config_dir).ok();
        Self { config }
    }

    /// Check if authentication is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Validate a token
    pub fn validate_token(&self, token: &str) -> bool {
        if !self.config.enabled {
            return true; // When disabled, all tokens are valid
        }
        
        if self.config.token.is_empty() {
            // Try to load from file
            if let Ok(saved_token) = self.load_token() {
                return token == saved_token;
            }
            return false;
        }
        
        token == self.config.token
    }

    /// Generate a new random token
    pub fn generate_token(&self) -> String {
        // Generate a 32-character random token
        let uuid1 = Uuid::new_v4().to_string().replace("-", "");
        let uuid2 = Uuid::new_v4().to_string().replace("-", "");
        format!("{}{}", uuid1, uuid2)[..32].to_string()
    }

    /// Save token to file
    pub fn save_token(&self, token: &str) -> Result<(), std::io::Error> {
        let token_path = self.config.config_dir.join("token");
        fs::write(token_path, token)
    }

    /// Load token from file
    pub fn load_token(&self) -> Result<String, std::io::Error> {
        let token_path = self.config.config_dir.join("token");
        fs::read_to_string(token_path)
    }
}
