use super::{canvas::write_canvas_items, function_graph, sbs_writer::SbsWriter, xml::xml_escape};
use crate::{FormatError, Result};
use materialcraft_core::Graph;
use std::fmt::Write;

pub(super) fn write(output: &mut String, graph: &Graph, writer: &mut SbsWriter) -> Result<()> {
    let function = graph.function.as_ref().ok_or_else(|| FormatError::Io("missing function graph".into()))?;
    let types = materialcraft_nodes::function::node_types(function).map_err(FormatError::Io)?;
    let result_type = function.output.and_then(|id| types.get(&id)).copied().unwrap_or(256);
    let mut body = function_graph::write(function, writer).map_err(FormatError::Io)?;
    body.truncate(body.len().saturating_sub("</dynamicValue>".len()));
    write_canvas_items(&mut body, graph, writer);
    body.push_str("</dynamicValue>");

    let _ = write!(output, "<function><identifier v=\"{}\"/><uid v=\"{}\"/>", xml_escape(&graph.name), writer.next_uid());
    let attributes = &graph.attributes;
    let _ = write!(
        output,
        "<attributes><category v=\"{}\"/><description v=\"{}\"/><author v=\"{}\"/><tags v=\"{}\"/></attributes>",
        xml_escape(&attributes.category),
        xml_escape(&attributes.description),
        xml_escape(&attributes.author),
        xml_escape(&attributes.tags)
    );
    writer.write_graph_inputs(output, graph);
    let _ = write!(output, "<type v=\"{result_type}\"/><paramValue>{body}</paramValue></function>");
    Ok(())
}
