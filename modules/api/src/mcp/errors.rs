use crate::errors::ApiError;
use axum::http::StatusCode;
use exoharness_mcp::McpError;
impl From<McpError> for ApiError {
    fn from(error: McpError) -> Self {
        match error {
            McpError::Store(error) => error.into(),
            McpError::Import(message) => {
                Self(StatusCode::BAD_REQUEST, "invalid_mcp_import", message)
            }
            McpError::Configuration | McpError::Arguments => Self(
                StatusCode::BAD_REQUEST,
                "invalid_mcp_configuration",
                "MCP 配置或参数无效；未调用工具",
            ),
            McpError::Permission => Self(
                StatusCode::FORBIDDEN,
                "permission_required",
                "声明权限尚未全部允许；未调用工具",
            ),
            McpError::SandboxRequired => Self(
                StatusCode::CONFLICT,
                "sandbox_required",
                "本地 MCP 已登记，但受管沙盒启动器尚未就绪；未在宿主机执行",
            ),
            McpError::Secret => Self(
                StatusCode::CONFLICT,
                "secret_required",
                "认证信息未配置或格式无效，请在能力设置中更新；未调用工具",
            ),
            McpError::Cancelled => Self(
                StatusCode::CONFLICT,
                "cancelled",
                "调用已取消，已提交的外部结果仍需核对",
            ),
            McpError::OutcomeUnknown => Self(
                StatusCode::CONFLICT,
                "outcome_unknown",
                "调用结果未知，不会自动重新提交",
            ),
            McpError::Rejected(_) | McpError::Connection | McpError::Protocol => Self(
                StatusCode::BAD_GATEWAY,
                "mcp_protocol_failed",
                "MCP 连接或协议返回未完成，不会自动重投业务调用",
            ),
        }
    }
}
