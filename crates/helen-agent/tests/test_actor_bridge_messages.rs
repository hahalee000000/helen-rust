//! Tests for actor bridge message types

use helen_agent::actor_bridge::messages::{AgentOutput, StreamChunk, UserInput};

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
fn test_user_input_with_file_paths() {
    let input = UserInput {
        content: "Process this file".to_string(),
        file_paths: vec!["/path/to/file.txt".to_string()],
        request_id: "req-2".to_string(),
    };
    assert_eq!(input.file_paths.len(), 1);
    assert_eq!(input.file_paths[0], "/path/to/file.txt");
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
fn test_agent_output_error() {
    let output = AgentOutput::Error {
        request_id: "req-1".to_string(),
        error_msg: "Something went wrong".to_string(),
    };
    match output {
        AgentOutput::Error { error_msg, .. } => {
            assert_eq!(error_msg, "Something went wrong");
        }
        _ => panic!("Expected Error"),
    }
}

#[test]
fn test_agent_output_actor_status() {
    let output = AgentOutput::ActorStatus {
        status: "exited".to_string(),
    };
    match output {
        AgentOutput::ActorStatus { status } => {
            assert_eq!(status, "exited");
        }
        _ => panic!("Expected ActorStatus"),
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

#[test]
fn test_stream_chunk_sequence_ordering() {
    let chunk1 = StreamChunk {
        sequence: 1,
        content: "First".to_string(),
    };
    let chunk2 = StreamChunk {
        sequence: 2,
        content: "Second".to_string(),
    };
    assert!(chunk1.sequence < chunk2.sequence);
}
