#![cfg(feature = "live-provider")]
use living_world_core::provider::{ProviderTransport, UreqTransport};
use serde_json::{json, Value};
use std::{io::{Read, Write}, net::TcpListener, thread};

fn response(status: u16, body: String) -> Result<Value, living_world_core::provider::ProviderError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/generate", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0; 8192];
        let _ = stream.read(&mut buffer).unwrap();
        let header = format!("HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body.as_bytes());
    });
    let result = UreqTransport.post_json(&endpoint, "test-secret-key", &json!({"prompt":"private input"}));
    server.join().unwrap();
    result
}

#[test]
fn captures_sanitized_google_400_through_actual_http() {
    let error = response(400, json!({"error":{"code":400,"status":"INVALID_ARGUMENT","message":"Invalid schema; key=test-secret-key","details":[{"reason":"SCHEMA_VALIDATION","fieldViolations":[{"field":"generationConfig.responseJsonSchema","description":"Too complex"}]}]},"request":{"prompt":"private input"}}).to_string()).unwrap_err();
    let diagnostic = error.response.expect("HTTP 400 must retain diagnostic fields");
    assert_eq!(diagnostic["httpStatus"], 400);
    assert_eq!(diagnostic["error"]["status"], "INVALID_ARGUMENT");
    assert!(diagnostic.to_string().contains("SCHEMA_VALIDATION"));
    assert!(!diagnostic.to_string().contains("test-secret-key"));
    assert!(!diagnostic.to_string().contains("private input"));
}

#[test]
fn successful_json_is_not_modified_and_invalid_json_stays_an_error() {
    let body = json!({"candidates":[{"content":{"parts":[{"text":"{}"}]}}]});
    assert_eq!(response(200, body.to_string()).unwrap(), body);
    assert!(response(200, "not JSON".into()).is_err());
}

#[test]
fn oversized_error_is_bounded_and_keeps_http_status() {
    let error = response(400, "x".repeat(100_000)).unwrap_err();
    let diagnostic = error.response.expect("status must survive oversized bodies");
    assert_eq!(diagnostic["httpStatus"], 400);
    assert!(diagnostic.to_string().len() < 20_000);
}

#[test]
fn successful_generation_larger_than_diagnostic_limit_is_retained() {
    let body = json!({"candidates":[{"content":{"parts":[{"text":"x".repeat(100_000)}]}}]});
    assert_eq!(response(200, body.to_string()).unwrap(), body);
}
