// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP 配置导入参数无效：{0}")]
    Import(&'static str),
    #[error(transparent)]
    Store(#[from] exoharness_storage_postgres::StoreError),
    #[error("能力尚未启用或未获得全部声明权限")]
    Permission,
    #[error("这不是有效的 MCP 配置")]
    Configuration,
    #[error("本地 MCP 必须通过受管沙盒启动，不能在宿主机运行")]
    SandboxRequired,
    #[error("MCP 密钥未配置或 Header 无效")]
    Secret,
    #[error("MCP 初始化或发现未完成")]
    Connection,
    #[error("请求已取消；已提交调用的外部结果仍需核对")]
    Cancelled,
    #[error("MCP 调用结果未知，不能自动重新提交")]
    OutcomeUnknown,
    #[error("MCP 返回了不匹配的协议响应")]
    Protocol,
    #[error("工具名或参数格式无效，未提交执行")]
    Arguments,
    #[error("MCP 返回协议错误（代码 {0}）")]
    Rejected(i32),
}
pub type Result<T> = std::result::Result<T, McpError>;
