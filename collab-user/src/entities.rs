use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::reminder::Reminder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAwarenessData {
  pub appearance_settings: HashMap<String, String>,
  pub reminders: Vec<Reminder>,
}
