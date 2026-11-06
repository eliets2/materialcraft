use crate::MaterialCraftApp;
use serde_json::Value;

pub(crate) fn invoke(app: &mut MaterialCraftApp, selection: &mut Vec<u32>, command: &str, parameters: Value) {
    if let Some(mut result) = app.run(command, parameters)
        && let Some(ids) = result.get_mut("ids")
        && let Ok(updated) = serde_json::from_value(ids.take())
    {
        *selection = updated;
    }
}
