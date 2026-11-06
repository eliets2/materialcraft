use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MenuParams {
    /// Menu item id from the app's `ui.menu.list`, e.g. `file.save`, `ui.newGraph`, `ui.view.library`
    pub id: String,
}
