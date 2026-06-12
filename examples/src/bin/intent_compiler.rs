use serde_json::json;
use zap_intent::{
    IntentPolicy, IntentPolicyDecision, IntentPolicyRule, compile_intent, explain_intent,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ZAP Deterministic Intent Compiler & Policy Engine ===");

    // 1. Compile a simple intent (natural language to structured action)
    let intent_a = "echo Hello from the local intent compiler";
    let plan_a = compile_intent(intent_a)?;
    println!("\n1. Compiling Simple Text Intent: '{}'", intent_a);
    println!("  Compiler ID: {}", plan_a.compiler);
    println!("  Steps Generated: {}", plan_a.steps.len());
    println!("  Step 1 Action: {}", plan_a.steps[0].action);
    println!("  Step 1 Payload: '{}'", plan_a.steps[0].payload);
    println!(
        "  Requires Consensus (PoA)? {}",
        plan_a.steps[0].requires_consensus
    );

    // 2. Compile an intent that resolves to multiple actions (multi-step planning)
    // ZAP can extract multiple actions from a single compound sentence.
    let intent_b = "Ajuster la temperature de la piece a 22 et declencher arret urgence robot";
    let plan_b = compile_intent(intent_b)?;
    println!("\n2. Compiling Compound French Intent: '{}'", intent_b);
    println!("  Steps Generated: {}", plan_b.steps.len());
    for (i, step) in plan_b.steps.iter().enumerate() {
        println!("  Step {}:", i + 1);
        println!("    Action: {}", step.action);
        println!("    Payload: {}", step.payload);
        println!("    Consensus (PoA) required? {}", step.requires_consensus);
        println!("    Rationale: {}", step.rationale);
    }
    println!("  Notes: {:?}", plan_b.notes);

    // 3. Explain the intent compilation (shows underlying rules applied)
    let explanation = explain_intent("Ajuster la temperature a 19")?;
    println!("\n3. Intent Explanation (Audit Mode):");
    println!("  Normalized String: '{}'", explanation.normalized);
    println!("  Matched Rules:");
    for rule in explanation.rules {
        println!("    - Rule '{}': {}", rule.id, rule.description);
    }

    // 4. Define and apply an Intent Policy
    // Policies act as safety gates that can Allow, Deny, or Require Proof-of-Action (PoA)
    // for specific actions before they are built into wire frames.
    let policy = IntentPolicy {
        default_decision: IntentPolicyDecision::Allow,
        rules: vec![
            // Rule 1: Temperature changes require Proof-of-Action consensus (RequirePoa)
            IntentPolicyRule {
                kind: Some("action".to_string()),
                subject: Some("thermostat.setpoint".to_string()),
                action: None,
                decision: IntentPolicyDecision::RequirePoa,
                reason: Some(
                    "temperature modifications must be verified by validators".to_string(),
                ),
            },
            // Rule 2: Deny any 'echo' actions in production environment
            IntentPolicyRule {
                kind: None,
                subject: Some("echo".to_string()),
                action: None,
                decision: IntentPolicyDecision::Deny,
                reason: Some("echo commands are disabled in production policy".to_string()),
            },
        ],
    };

    println!("\n4. Applying Safety Policy:");

    // Test Case A: Thermostat Setpoint (Should be upgraded to require PoA)
    let mut plan_thermostat = compile_intent("Ajuster la temperature a 20")?;
    println!(
        "  Thermostat Plan before policy: requires_consensus = {}",
        plan_thermostat.steps[0].requires_consensus
    );
    let report_thermostat = plan_thermostat.apply_policy(&policy)?;
    println!(
        "  Thermostat Plan after policy:  requires_consensus = {}",
        plan_thermostat.steps[0].requires_consensus
    );
    println!(
        "  Policy Report Decision: {:?}",
        report_thermostat.decisions[0].decision
    );
    println!("  Reason: {}", report_thermostat.decisions[0].reason);

    // Test Case B: Echo (Should be rejected/denied)
    let mut plan_echo = compile_intent("echo test policy")?;
    print!("  Applying policy to 'echo' command: ");
    match plan_echo.apply_policy(&policy) {
        Ok(_) => println!("Allowed (Unsafe!)"),
        Err(e) => println!("Blocked by policy successfully! (Error: {})", e),
    }

    // 5. Compile structured JSON intents directly
    // This allows clients to bypass natural language and pass strict machine-readable intents.
    let json_intent = json!({
        "action": "thermostat.setpoint",
        "payload": { "temperature_c": 18.0 },
        "requires_consensus": true
    });
    let plan_json = compile_intent(&json_intent.to_string())?;
    println!("\n5. Structured JSON Intent Compiled:");
    println!("  Action: {}", plan_json.steps[0].action);
    println!("  Payload: {}", plan_json.steps[0].payload);
    println!("  Consensus: {}", plan_json.steps[0].requires_consensus);

    Ok(())
}
