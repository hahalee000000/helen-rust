//! Helen Agent WebUI — pure Rust web server for Helen agent
//!
//! This crate provides a web-based interface for the Helen agent system,
//! eliminating the need for Python dependencies in production use.

pub mod actor_bridge;
pub mod api;
pub mod auth;
pub mod directory;
pub mod executor;
pub mod helen_bridge;
pub mod server;
pub mod stream_registry;
pub mod transcript;
pub mod upload;
pub mod websocket;
