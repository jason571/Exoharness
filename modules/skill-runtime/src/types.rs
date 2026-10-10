use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentEvidence {
    pub capability_id: String,
    pub revision: i64,
    pub source_digest: String,
    pub image_id: String,
    pub ready: bool,
    pub runtimes: Vec<String>,
    pub missing: Vec<String>,
    pub versions: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Serialize)]
pub struct RuntimeStatus {
    pub scripts_present: bool,
    pub environment_ready: bool,
    /// An environment probe is not an independently registered Tool grant.
    pub script_execution_ready: bool,
    pub evidence: Option<EnvironmentEvidence>,
}
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("Skill 版本已变化，未沿用旧运行环境证明")]
    Conflict,
    #[error("未获授权准备该 Skill；没有运行上传代码")]
    Permission,
    #[error("运行依赖声明不受支持或超过检查限制；未安装依赖")]
    Declaration,
    #[error("Skill 运行环境检查暂不可用；不会回退宿主机")]
    Unavailable,
    #[error("Skill 运行环境证明无法读取或安全保存")]
    Storage,
    #[error("{0}")]
    Skill(#[from] exoharness_skills::SkillError),
    #[error("{0}")]
    Store(#[from] exoharness_storage_postgres::StoreError),
    #[error("{0}")]
    Sandbox(#[from] exoharness_sandbox::SandboxError),
}
