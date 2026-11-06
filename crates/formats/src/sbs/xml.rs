use roxmltree::Node as XmlNode;

pub(super) fn parse_floats(text: &str) -> Vec<f32> {
    text.split_whitespace().take(16).map(|t| t.parse::<f32>().ok().filter(|v| v.is_finite()).unwrap_or(0.0)).collect()
}
pub(super) fn parse_ints(text: &str) -> Vec<i32> {
    let parse = |t: &str| t.parse::<i64>().map(|v| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32).or_else(|_| t.parse::<f64>().map(|v| v as i32));
    text.split_whitespace().take(16).map(|t| parse(t).unwrap_or(0)).collect()
}
pub(super) fn format_float(v: f32) -> String {
    if v.is_finite() { format!("{v}") } else { "0".into() }
}

pub(super) fn child_element<'a, 'input>(node: XmlNode<'a, 'input>, tag: &str) -> Option<XmlNode<'a, 'input>> {
    node.children().find(|c| c.is_element() && c.tag_name().name() == tag)
}

pub(super) fn children_named<'a, 'input: 'a>(node: XmlNode<'a, 'input>, tag: &'a str) -> impl Iterator<Item = XmlNode<'a, 'input>> + 'a {
    node.children().filter(move |c| c.is_element() && c.tag_name().name() == tag)
}
pub(super) fn child_value<'a>(node: XmlNode<'a, '_>, tag: &str) -> Option<&'a str> {
    child_element(node, tag).and_then(|c| c.attribute("v"))
}
pub(super) fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c if (c as u32) < 0x20 || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}
pub(super) fn is_parameters_element(xml: &str) -> bool {
    let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 1_000_000 };
    xml.len() < (16 << 20) && roxmltree::Document::parse_with_options(xml, options).is_ok_and(|d| d.root_element().tag_name().name() == "parameters")
}
