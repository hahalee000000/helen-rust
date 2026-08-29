//! Message types for Rust ↔ Helen communication
//!
//! These types define the protocol between the Rust web server
//! and the Helen ChatSessionActor.

/// User input sent to ChatSessionActor
#[derive(Debug, Clone)]
pub struct UserInput {
    /// The message content from the user
    pub content: String,
    /// Optional file paths attached to the message
    pub file_paths: Vec<String>,
    /// Unique request ID for matching responses
    pub request_id: String,
}

/// Agent output received from ChatSessionActor
#[derive(Debug, Clone)]
pub enum AgentOutput {
    /// Complete response from the agent
    ResponseComplete {
        /// Request ID this response corresponds to
        request_id: String,
        /// The full response content
        content: String,
    },
    /// Error occurred during processing
    Error {
        /// Request ID this error corresponds to
        request_id: String,
        /// Error message
        error_msg: String,
    },
    /// Actor status change (e.g., exited)
    ActorStatus {
        /// Status description
        status: String,
    },
}

/// Streaming chunk for real-time output
#[derive(Debug, Clone)]
pub struct StreamChunk {
    /// Sequence number for ordering
    pub sequence: u64,
    /// Chunk content
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_input_creation() {
        let input = UserInput {
            content: "Hello".to_string(),
            file_paths: vec![],
            request_id: "req-1".to_string(),
        };
        assert_eq!(input.content, "Hello");
        assert_eq!(input.request_id, "req-1");
        assert!(input.file_paths.is_empty());
    }

    #[test]
    fn test_agent_output_response_complete() {
        let output = AgentOutput::ResponseComplete {
            request_id: "req-1".to_string(),
            content: "Hi there".to_string(),
        };
        match output {
            AgentOutput::ResponseComplete { content, .. } => {
                assert_eq!(content, "Hi there");
            }
            _ => panic!("Expected ResponseComplete"),
        }
    }

    #[test]
    fn test_stream_chunk_creation() {
        let chunk = StreamChunk {
            sequence: 1,
            content: "Hello ".to_string(),
        };
        assert_eq!(chunk.sequence, 1);
        assert_eq!(chunk.content, "Hello ");
    }
}
