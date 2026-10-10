use rmcp::{
    ClientHandler,
    model::{ClientInfo, ProgressNotificationParam},
    service::{NotificationContext, RoleClient},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub token: serde_json::Value,
    pub progress: f64,
    pub total: Option<f64>,
    pub message: Option<String>,
}
pub(crate) struct Handler {
    pub progress: broadcast::Sender<Progress>,
}
impl ClientHandler for Handler {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        let Ok(token) = serde_json::to_value(params.progress_token) else {
            return;
        };
        let _ = self.progress.send(Progress {
            token,
            progress: params.progress,
            total: params.total,
            message: params.message,
        });
    }
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}
