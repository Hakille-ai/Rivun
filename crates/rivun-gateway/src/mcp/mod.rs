//! Model Context Protocol (MCP) Server Implementation.

pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod stdio;
pub mod tools;

pub use stdio::McpStdioTransport;

use serde_json::{Value, json};
use std::sync::Arc;
use tracing::warn;

use crate::error::RivunGatewayError;
use crate::mcp::protocol::*;
use crate::mcp::tools::ToolExecutionContext;

#[derive(Clone)]
pub struct McpEngine {
    ctx: Arc<ToolExecutionContext>,
}

impl McpEngine {
    pub fn new(ctx: ToolExecutionContext) -> Self {
        Self { ctx: Arc::new(ctx) }
    }

    pub fn context(&self) -> &ToolExecutionContext {
        &self.ctx
    }

    pub async fn handle_jsonrpc_str(&self, input: &str) -> String {
        let req_val: Value = match serde_json::from_str(input) {
            Ok(v) => v,
            Err(err) => {
                let err_resp = JsonRpcResponse::error(
                    None,
                    JsonRpcError::parse_error(format!("JSON parse error: {err}")),
                );
                return serde_json::to_string(&err_resp).unwrap_or_default();
            }
        };

        let response = self.handle_jsonrpc_value(req_val).await;
        serde_json::to_string(&response).unwrap_or_default()
    }

    pub async fn handle_jsonrpc_value(&self, req_val: Value) -> JsonRpcResponse {
        let req: JsonRpcRequest = match serde_json::from_value(req_val) {
            Ok(r) => r,
            Err(err) => {
                return JsonRpcResponse::error(
                    None,
                    JsonRpcError::invalid_request(format!("Invalid JSON-RPC 2.0 request: {err}")),
                );
            }
        };

        if req.jsonrpc != "2.0" {
            return JsonRpcResponse::error(
                req.id,
                JsonRpcError::invalid_request("Field `jsonrpc` must be \"2.0\""),
            );
        }

        let id = req.id.clone();
        match req.method.as_str() {
            "initialize" => {
                let init_res = InitializeResult {
                    protocol_version: MCP_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities {
                        tools: json!({ "listChanged": true }),
                        resources: json!({ "subscribe": false, "listChanged": true }),
                        prompts: json!({ "listChanged": true }),
                    },
                    server_info: ServerInfo {
                        name: MCP_SERVER_NAME.to_string(),
                        version: MCP_SERVER_VERSION.to_string(),
                    },
                };
                match serde_json::to_value(init_res) {
                    Ok(val) => JsonRpcResponse::success(id, val),
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "tools/list" => {
                let tools_list = ToolsListResult {
                    tools: tools::list_tools(),
                };
                match serde_json::to_value(tools_list) {
                    Ok(val) => JsonRpcResponse::success(id, val),
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "tools/call" => {
                let params: ToolCallParams = match req.params {
                    Some(p) => match serde_json::from_value(p) {
                        Ok(call_params) => call_params,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                JsonRpcError::invalid_params(format!(
                                    "Invalid tool call params: {e}"
                                )),
                            );
                        }
                    },
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing params object for tools/call"),
                        );
                    }
                };

                match tools::execute_tool(params, &self.ctx).await {
                    Ok(result) => match serde_json::to_value(result) {
                        Ok(val) => JsonRpcResponse::success(id, val),
                        Err(e) => {
                            JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                        }
                    },
                    Err(RivunGatewayError::JsonRpc { code, message, .. }) => {
                        JsonRpcResponse::error(id, JsonRpcError::new(code, message))
                    }
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "resources/list" => {
                let resources_list = resources::list_resources();
                match serde_json::to_value(resources_list) {
                    Ok(val) => JsonRpcResponse::success(id, val),
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "resources/read" => {
                let params: ResourceReadParams = match req.params {
                    Some(p) => match serde_json::from_value(p) {
                        Ok(read_params) => read_params,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                JsonRpcError::invalid_params(format!(
                                    "Invalid resource read params: {e}"
                                )),
                            );
                        }
                    },
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params(
                                "Missing params object for resources/read",
                            ),
                        );
                    }
                };

                match resources::read_resource(params, &self.ctx).await {
                    Ok(result) => match serde_json::to_value(result) {
                        Ok(val) => JsonRpcResponse::success(id, val),
                        Err(e) => {
                            JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                        }
                    },
                    Err(RivunGatewayError::NotFound(msg)) => {
                        JsonRpcResponse::error(id, JsonRpcError::invalid_params(msg))
                    }
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "prompts/list" => {
                let prompts_list = prompts::list_prompts();
                match serde_json::to_value(prompts_list) {
                    Ok(val) => JsonRpcResponse::success(id, val),
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "prompts/get" => {
                let params: PromptGetParams = match req.params {
                    Some(p) => match serde_json::from_value(p) {
                        Ok(get_params) => get_params,
                        Err(e) => {
                            return JsonRpcResponse::error(
                                id,
                                JsonRpcError::invalid_params(format!(
                                    "Invalid prompt get params: {e}"
                                )),
                            );
                        }
                    },
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            JsonRpcError::invalid_params("Missing params object for prompts/get"),
                        );
                    }
                };

                match prompts::get_prompt(params) {
                    Ok(result) => match serde_json::to_value(result) {
                        Ok(val) => JsonRpcResponse::success(id, val),
                        Err(e) => {
                            JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                        }
                    },
                    Err(RivunGatewayError::NotFound(msg)) => {
                        JsonRpcResponse::error(id, JsonRpcError::invalid_params(msg))
                    }
                    Err(e) => {
                        JsonRpcResponse::error(id, JsonRpcError::internal_error(e.to_string()))
                    }
                }
            }

            "ping" => JsonRpcResponse::success(id, json!({})),

            unknown => {
                warn!("MCP unknown method called: {}", unknown);
                JsonRpcResponse::error(
                    id,
                    JsonRpcError::method_not_found(format!("Method not found: {unknown}")),
                )
            }
        }
    }
}
