//! Tests for authentication

use helen_agent::auth::{AuthConfig, AuthManager};
use tempfile::tempdir;

#[test]
fn test_create_auth_manager() {
    let dir = tempdir().unwrap();
    let config = AuthConfig {
        enabled: true,
        token: "test-token-123".to_string(),
        config_dir: dir.path().to_path_buf(),
    };

    let auth = AuthManager::new(config);
    assert!(auth.is_enabled());
}

#[test]
fn test_validate_token() {
    let dir = tempdir().unwrap();
    let config = AuthConfig {
        enabled: true,
        token: "test-token-123".to_string(),
        config_dir: dir.path().to_path_buf(),
    };

    let auth = AuthManager::new(config);
    assert!(auth.validate_token("test-token-123"));
    assert!(!auth.validate_token("wrong-token"));
}

#[test]
fn test_auth_disabled() {
    let dir = tempdir().unwrap();
    let config = AuthConfig {
        enabled: false,
        token: "".to_string(),
        config_dir: dir.path().to_path_buf(),
    };

    let auth = AuthManager::new(config);
    assert!(!auth.is_enabled());
    // When disabled, all tokens are valid
    assert!(auth.validate_token("any-token"));
}

#[test]
fn test_generate_token() {
    let dir = tempdir().unwrap();
    let config = AuthConfig {
        enabled: true,
        token: "".to_string(),
        config_dir: dir.path().to_path_buf(),
    };

    let auth = AuthManager::new(config);
    let token = auth.generate_token();
    assert!(!token.is_empty());
    assert!(token.len() >= 32);
}

#[test]
fn test_save_and_load_token() {
    let dir = tempdir().unwrap();
    let config = AuthConfig {
        enabled: true,
        token: "".to_string(),
        config_dir: dir.path().to_path_buf(),
    };

    let auth = AuthManager::new(config);
    let token = auth.generate_token();
    auth.save_token(&token).unwrap();

    let loaded = auth.load_token().unwrap();
    assert_eq!(loaded, token);
}
