use crate::commands::{CommandSpec, bad, has_graph, strp, u64p};
use crate::{AnnotationSelection, Result, Session, cmd};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.annotation.select",
        "Select annotation",
        [],
        None,
        "{kind?: frame|comment, id?} clears node selection; omit kind to deselect",
        has_graph,
        select
    )]
}

fn select(session: &mut Session, parameters: &Value) -> Result<Value> {
    let selection = match strp(parameters, "kind") {
        None => None,
        Some(kind) => {
            let id = u64p(parameters, "id").ok_or_else(|| bad("graph.annotation.select", "`id` is required"))?;
            let selection = match kind {
                "frame" => AnnotationSelection::Frame(id),
                "comment" => AnnotationSelection::Comment(id),
                _ => return Err(bad("graph.annotation.select", "kind must be frame or comment")),
            };
            if !selection.exists(session.graph()?) {
                return Err(bad("graph.annotation.select", "annotation does not exist"));
            }

            Some(selection)
        }
    };

    session.annotation_selection = selection;
    if selection.is_some() {
        session.selection.clear();
    }

    Ok(json!({"selection": selection}))
}
