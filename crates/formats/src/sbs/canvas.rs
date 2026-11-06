use super::sbs_writer::SbsWriter;
use super::warnings::Warnings;
use super::xml::{child_element, child_value, children_named, parse_floats};
use super::xml::{format_float, xml_escape};
use materialcraft_core::{Comment, Frame, Pin, srgb_to_linear};
use materialcraft_core::{Graph, linear_to_srgb};
use roxmltree::Node as XmlNode;
use std::fmt::Write as _;

pub(super) fn write_canvas_items(output: &mut String, graph: &Graph, writer: &mut SbsWriter) {
    output.push_str("<GUIObjects>");

    for frame in &graph.frames {
        let [x, y, width, height] = frame.rect.map(format_float);
        let [red, green, blue] = [0, 1, 2].map(|channel| format_float(linear_to_srgb(frame.color[channel])));
        let _ = write!(
            output,
            "<GUIObject><type v=\"COMMENT\"/><GUILayout><gpos v=\"{x} {y} -100\"/><size v=\"{width} {height}\"/></GUILayout><GUIName v=\"{}\"/><uid v=\"{}\"/><title v=\"{}\"/><frameColor v=\"{red} {green} {blue} {}\"/><isTitleVisible v=\"{}\"/><isFrameVisible v=\"1\"/></GUIObject>",
            xml_escape(&frame.description),
            writer.next_uid(),
            xml_escape(&frame.title),
            format_float(frame.color[3]),
            u8::from(frame.title_visible)
        );
    }

    for comment in &graph.comments {
        let [x, y] = comment.pos.map(format_float);
        let [width, height] = comment.size.map(format_float);
        let _ = write!(
            output,
            "<GUIObject><type v=\"COMMENT\"/><GUILayout><gpos v=\"{x} {y} 0\"/><size v=\"{width} {height}\"/></GUILayout><GUIName v=\"{}\"/><uid v=\"{}\"/><isFrameVisible v=\"0\"/></GUIObject>",
            xml_escape(&comment.text),
            writer.next_uid()
        );
    }

    for pin in &graph.pins {
        let [x, y] = pin.pos.map(format_float);
        let _ = write!(
            output,
            "<GUIObject><type v=\"PIN\"/><GUILayout><gpos v=\"{x} {y} 0\"/></GUILayout><GUIName v=\"{}\"/><uid v=\"{}\"/></GUIObject>",
            xml_escape(&pin.name),
            writer.next_uid()
        );
    }

    output.push_str("</GUIObjects>");
}

pub(super) fn read_canvas_items(element: XmlNode, graph: &mut Graph, warnings: &mut Warnings) {
    for item in child_element(element, "GUIObjects").into_iter().flat_map(|objects| children_named(objects, "GUIObject")) {
        let layout = child_element(item, "GUILayout");
        let position = layout.and_then(|layout| child_value(layout, "gpos")).map(parse_floats).unwrap_or_default();
        let size = layout.and_then(|layout| child_value(layout, "size")).map(parse_floats).unwrap_or_default();
        let x = position.first().copied().unwrap_or(0.0);
        let y = position.get(1).copied().unwrap_or(0.0);
        let width = size.first().copied().unwrap_or(180.0).max(8.0);
        let height = size.get(1).copied().unwrap_or(70.0).max(8.0);
        let text = child_value(item, "GUIName").unwrap_or("").to_string();

        match child_value(item, "type") {
            Some("COMMENT") if child_value(item, "isFrameVisible") == Some("1") => {
                let channels = child_value(item, "frameColor").map(parse_floats).unwrap_or_default();
                let color = match channels.as_slice() {
                    &[red, green, blue, alpha, ..] => [srgb_to_linear(red), srgb_to_linear(green), srgb_to_linear(blue), alpha],
                    _ => [0.2, 0.4, 0.8, 0.25],
                };
                let title = child_value(item, "title").unwrap_or("").to_string();
                let title_visible = child_value(item, "isTitleVisible") != Some("0");
                let id = graph.alloc_id();

                graph.frames.push(Frame { id, title, title_visible, description: text, rect: [x, y, width, height], color });
            }
            Some("COMMENT") => {
                let id = graph.alloc_id();
                graph.comments.push(Comment { id, text, pos: [x, y], size: [width, height] });
            }
            Some("PIN") => {
                let id = graph.alloc_id();
                graph.pins.push(Pin { id, name: text, pos: [x, y] });
            }
            Some(other) => warnings.add(format!("graph `{}`: canvas item `{other}` is not supported", graph.name)),
            None => {}
        }
    }
}
