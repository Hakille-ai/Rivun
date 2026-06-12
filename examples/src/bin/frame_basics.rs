use bytes::Bytes;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, sign_frame, verify_frame};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ZAP Binary Frame Basics ===");

    // 1. Generate cryptographic identities (Ed25519)
    let alice_keys = Keypair::generate();
    let bob_keys = Keypair::generate();

    let alice_node_id = alice_keys.node_id();
    let bob_node_id = bob_keys.node_id();

    println!("Alice Node ID (UUID v8 derived from public key): {}", alice_node_id);
    println!("Bob Node ID: {}", bob_node_id);

    // 2. Build an unsigned frame containing a payload
    let payload = Bytes::from("action:thermostat.setpoint temperature_c=22.5");
    let unsigned_frame = ZapFrame::new(
        alice_node_id,                 // Source Node ID
        bob_node_id,                   // Target Node ID
        ZapFlags::PRIORITY,            // Frame flags (e.g. priority, broadcast, encrypted)
        payload.clone(),               // Binary payload
    )?;

    println!("\nUnsigned Frame Header:");
    println!("  Magic: ZAP_");
    println!("  Version: {}", unsigned_frame.header.version);
    println!("  Flags: {:?}", unsigned_frame.header.flags);
    println!("  Timestamp (µs): {}", unsigned_frame.header.timestamp_micros);
    println!("  Payload Length: {} bytes", unsigned_frame.header.zap_len);

    // 3. Sign the frame using Alice's private key
    // This updates the frame flags, sets a signature hint in the header, and attaches an Ed25519 signature trailer.
    let signed_frame = sign_frame(&alice_keys, &unsigned_frame)?;

    println!("\nSigned Frame Header:");
    println!("  Flags (now includes SIGNED): {:?}", signed_frame.header.flags);
    println!("  Signature Hint (8 bytes): {:x?}", signed_frame.header.zap_sign);
    if let Some(auth) = &signed_frame.auth {
        println!("  Ed25519 Signature (64 bytes): {:x?}...", &auth.signature[..16]);
    }

    // 4. Serialize the frame to binary format (ready for transmission over UDP)
    let binary_data = signed_frame.encode();
    println!("\nEncoded frame size: {} bytes", binary_data.len());

    // 5. Simulate network transmission / reception
    // Decode the binary data back into a ZapFrame struct on the receiver's end
    let decoded_frame = ZapFrame::decode(&binary_data)?;
    println!("\nDecoded frame: matches original payload? {}", decoded_frame.payload == payload);

    // 6. Cryptographically verify the frame using Alice's public key
    // This checks both the fast-filtering signature hint and the full 64-byte Ed25519 signature.
    match verify_frame(&alice_keys.verifying_key(), &decoded_frame) {
        Ok(()) => {
            println!("Verification succeeded: Frame is authentic and untampered!");
        }
        Err(e) => {
            println!("Verification failed: {}", e);
            return Err(e.into());
        }
    }

    // 7. Demonstrate tampering protection
    // Let's modify the payload of the decoded frame and check if verification catches it
    let mut tampered_frame = decoded_frame.clone();
    tampered_frame.payload = Bytes::from("action:thermostat.setpoint temperature_c=99.9"); // Malicious setpoint
    
    println!("\nSimulating tampering...");
    match verify_frame(&alice_keys.verifying_key(), &tampered_frame) {
        Ok(()) => {
            println!("CRITICAL SECURITY FAILURE: Tampered frame verified successfully!");
        }
        Err(_) => {
            println!("Security check working: Tampered frame rejected successfully!");
        }
    }

    Ok(())
}
