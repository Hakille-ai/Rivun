use bytes::Bytes;
use uuid::Uuid;
use zap_envelope::{ZapEnvelope, ZapEnvelopeRef, ZapMessageKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ZAP Universal Payload Envelopes (ZENV) ===");

    // 1. Create an Action Envelope (synchronous command intended for a driver)
    // Actions are usually routed to local drivers or remote nodes running drivers.
    let action_body = Bytes::from(r#"{"action": "set_target", "temp_c": 22.0}"#);
    let action_id = Uuid::new_v4();
    let action_env = ZapEnvelope::action("thermostat.control", action_body)?
        .with_id(action_id)
        .with_content_type("application/json")?
        .with_metadata(Bytes::from("auth_token=super-secret-token"))?;

    println!("\n1. Action Envelope Created:");
    println!("  Kind: {:?}", action_env.kind());
    println!("  ID: {}", action_env.id());
    println!("  Subject: {}", action_env.subject());
    println!("  Content-Type: {}", action_env.content_type());
    println!("  Metadata: {}", String::from_utf8_lossy(action_env.metadata()));
    println!("  Body: {}", String::from_utf8_lossy(action_env.body()));

    // 2. Create an Event Envelope (asynchronous notification)
    // We link this Event to the previous Action using correlation and causation IDs
    // to build an explainable causal trace of system decisions.
    let event_body = Bytes::from(r#"{"status": "heating", "current_temp_c": 19.5}"#);
    let event_env = ZapEnvelope::event("thermostat.status", event_body)?
        .with_correlation_id(action_id) // Linked to the initial request
        .with_causation_id(action_id)   // Directly caused by the execution of the action
        .with_content_type("application/json")?;

    println!("\n2. Event Envelope (Correlated with Action):");
    println!("  Kind: {:?}", event_env.kind());
    println!("  ID: {}", event_env.id());
    println!("  Correlation ID: {:?}", event_env.correlation_id());
    println!("  Causation ID: {:?}", event_env.causation_id());
    println!("  Subject: {}", event_env.subject());

    // 3. Serialize and Deserialize (zero-copy parsing via ZapEnvelopeRef)
    // ZAP uses zero-copy parsing where possible for maximum speed and lowest memory consumption.
    let binary_payload = event_env.encode();
    println!("\nEncoding event envelope: {} bytes on wire", binary_payload.len());

    let parsed_ref = ZapEnvelopeRef::parse(&binary_payload)?;
    println!("\n3. Zero-Copy Parsed Envelope:");
    println!("  Parsed Kind: {:?}", parsed_ref.kind());
    println!("  Parsed Subject: {}", parsed_ref.subject());
    println!("  Parsed Body: {}", String::from_utf8_lossy(parsed_ref.body()));

    // 4. Show the 8 message kinds supported by ZAP
    println!("\n4. ZAP-Wire Universal Message Kinds:");
    let kinds = [
        ZapMessageKind::Data,        // Raw byte payloads (no subject required)
        ZapMessageKind::Event,       // Broadcast/subscribable events
        ZapMessageKind::Command,     // Direct commands
        ZapMessageKind::Query,       // Read queries
        ZapMessageKind::Response,    // Query responses
        ZapMessageKind::StreamChunk, // Multimedia or file chunks
        ZapMessageKind::Action,      // Cognitive WASM actions
        ZapMessageKind::Control,     // Internal node orchestration (e.g. key exchange, discovery)
    ];

    for kind in kinds {
        println!("  - {:<12} (Needs subject? {})", kind.as_str(), kind.requires_subject());
    }

    Ok(())
}
