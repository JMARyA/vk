use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub token: String,
    /// The user vk acts as for `claim`, `unclaim` and `--mine`. Only needed
    /// with API tokens, which cannot ask the server who they belong to.
    #[serde(default)]
    pub user_id: Option<i32>,
}
