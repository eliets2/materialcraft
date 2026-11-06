use crate::commands::{CommandSpec, always};
use crate::{Result, Session, cmd};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!("file.newAtomicSample", "Atomic sample", ["File", "New"], None, "{} opens an original graph made only of native primitives", always, create)]
}

fn create(session: &mut Session, _: &Value) -> Result<Value> {
    let package = crate::atomic_sample()?;
    crate::file_cmds::replace_package(session, package, None);
    Ok(json!({"graph": "Atomic_sample"}))
}
