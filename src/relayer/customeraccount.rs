use serde_derive::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TraderOrder {
    pub pk: String,
    pub auth: String,
    pub comm: String,
    pub sign: String,
    pub relayer_lock: String,
    pub is_relayer_lock_active: String,
}
