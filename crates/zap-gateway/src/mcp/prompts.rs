//! MCP Prompt templates and handlers.

use crate::error::{Result, ZapGatewayError};
use crate::mcp::protocol::{
    PromptArgument, PromptDescriptor, PromptGetParams, PromptGetResult, PromptMessage,
    PromptsListResult, ToolContent,
};

pub fn list_prompts() -> PromptsListResult {
    PromptsListResult {
        prompts: vec![
            PromptDescriptor {
                name: "goal_decomposition".to_string(),
                description: "Decompose a high-level goal into structured deterministic ZAP agent actions and constraints.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "objective".to_string(),
                        description: "High-level goal or task description to decompose".to_string(),
                        required: true,
                    },
                ],
            },
            PromptDescriptor {
                name: "capability_negotiation".to_string(),
                description: "Negotiate required execution capabilities and permissions across multi-agent fleet.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "target_agent".to_string(),
                        description: "Agent ID to negotiate capabilities with".to_string(),
                        required: true,
                    },
                ],
            },
            PromptDescriptor {
                name: "safe_execution_verification".to_string(),
                description: "Verify policy rules, Proof-of-Action consensus threshold, and cryptographic receipts before actuation.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "action".to_string(),
                        description: "Proposed action or command name".to_string(),
                        required: true,
                    },
                ],
            },
            PromptDescriptor {
                name: "agent_action_plan".to_string(),
                description: "Generate structured deterministic action plan for ZAP AI agent execution.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "objective".to_string(),
                        description: "Task or goal to decompose".to_string(),
                        required: true,
                    },
                ],
            },
            PromptDescriptor {
                name: "policy_check".to_string(),
                description: "Validate proposed actions against node safety and capability constraints.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "action".to_string(),
                        description: "Action name".to_string(),
                        required: true,
                    },
                ],
            },
            PromptDescriptor {
                name: "incident_diagnostics".to_string(),
                description: "Analyze cluster incident evidence and recommend mitigation steps.".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "incident_id".to_string(),
                        description: "Incident snapshot UUID".to_string(),
                        required: false,
                    },
                ],
            },
        ],
    }
}

pub fn get_prompt(params: PromptGetParams) -> Result<PromptGetResult> {
    match params.name.as_str() {
        "goal_decomposition" | "agent_action_plan" => {
            let objective = params
                .arguments
                .get("objective")
                .and_then(|o| o.as_str())
                .unwrap_or("Execute multi-agent workflow");

            Ok(PromptGetResult {
                description: "ZAP Goal Decomposition & Action Plan Template".to_string(),
                messages: vec![PromptMessage {
                    role: "system".to_string(),
                    content: ToolContent {
                        content_type: "text".to_string(),
                        text: format!(
                            "You are a deterministic ZAP AI Agent. Goal: {objective}. Ensure all subtasks have explicit capabilities, required constraints, and signed receipt verification."
                        ),
                    },
                }],
            })
        }
        "capability_negotiation" => {
            let target = params
                .arguments
                .get("target_agent")
                .and_then(|t| t.as_str())
                .unwrap_or("peer_agent");

            Ok(PromptGetResult {
                description: "ZAP Capability Negotiation Template".to_string(),
                messages: vec![PromptMessage {
                    role: "system".to_string(),
                    content: ToolContent {
                        content_type: "text".to_string(),
                        text: format!(
                            "Negotiate capabilities with target agent `{target}`. Identify required permissions, optional tools, and expiration timeout."
                        ),
                    },
                }],
            })
        }
        "safe_execution_verification" | "policy_check" => {
            let action = params
                .arguments
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("unknown");

            Ok(PromptGetResult {
                description: "ZAP Safe Execution Verification Template".to_string(),
                messages: vec![PromptMessage {
                    role: "system".to_string(),
                    content: ToolContent {
                        content_type: "text".to_string(),
                        text: format!(
                            "Check policy constraints for action `{action}`. Verify capability requirements, proof-of-action threshold, and generate cryptographic provenance linkage."
                        ),
                    },
                }],
            })
        }
        "incident_diagnostics" => Ok(PromptGetResult {
            description: "ZAP Incident Diagnostics Template".to_string(),
            messages: vec![PromptMessage {
                role: "system".to_string(),
                content: ToolContent {
                    content_type: "text".to_string(),
                    text: "Analyze snapshot telemetry, replay protection state, and cryptographic provenance errors.".to_string(),
                },
            }],
        }),
        other => Err(ZapGatewayError::NotFound(format!(
            "Prompt template not found: {other}"
        ))),
    }
}
