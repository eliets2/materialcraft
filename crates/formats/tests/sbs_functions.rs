//! Original synthetic XML fixtures exercise structural SBS import without a native payload.
use materialcraft_core::Value;
use materialcraft_formats::sbs::{SbsRead, read_sbs, write_sbs};
use std::collections::BTreeMap;

fn node(uid: &str, op: &str, value: &str, connections: &str) -> String {
    let data = if value.is_empty() { String::new() } else { format!("<funcDatas><funcData><name v=\"{op}\"/>{value}</funcData></funcDatas>") };
    format!("<paramNode><uid v=\"{uid}\"/><function v=\"{op}\"/>{data}<connections>{connections}</connections></paramNode>")
}
fn wire(port: &str, uid: &str) -> String {
    format!("<connection><identifier v=\"{port}\"/><connRef v=\"{uid}\"/></connection>")
}
fn xml(nodes: &str, root: &str, filter: &str, parameter: &str) -> String {
    format!(
        r#"<package><content><graph><identifier v="Math"/><paraminputs><paraminput><identifier v="amount"/><uid v="90"/><type v="256"/><defaultValue><constantValueFloat1 v="0.75"/></defaultValue></paraminput></paraminputs><compNodes><compNode><uid v="10"/><compImplementation><compFilter><filter v="{filter}"/><parameters><parameter><name v="{parameter}"/><paramValue><dynamicValue><rootnode v="{root}"/><paramNodes>{nodes}</paramNodes></dynamicValue></paramValue></parameter></parameters></compFilter></compImplementation></compNode></compNodes></graph></content></package>"#
    )
}
fn read(text: &str) -> SbsRead {
    let report = read_sbs(text.as_bytes(), "math.sbs", &|_| None).unwrap();
    assert!(!report.exact);
    report
}
fn expression() -> String {
    // XML order and connection order deliberately differ from dependency/operand order.
    [
        node("99999999999", "sub", "", &(wire("b", "2") + &wire("a", "1"))),
        node("2", "const_float1", r#"<constantValue><constantValueFloat1 v="0.25"/></constantValue>"#, ""),
        node("1", "get_float1", r#"<constantValueString v="amount"/>"#, ""),
    ]
    .concat()
}
#[test]
fn imports_arithmetic_and_keeps_exposed_inputs_live() {
    let report = read(&xml(&expression(), "99999999999", "blur", "intensity"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let f = &report.pkg.graphs[0].nodes[0].functions["intensity"];
    assert_eq!(f.nodes.len(), 3);
    let mut vars = BTreeMap::new();
    for (input, expected) in [(0.75, 0.5), (0.5, 0.25)] {
        vars.insert("amount".into(), Value::Float(input));
        let program = materialcraft_nodes::function::compile(f, &vars, 0).unwrap();
        assert_eq!(program.eval([0.0; 2], [1.0; 2], &[], true, &mut Vec::new()), [expected; 4]);
    }
    let saved = write_sbs(&report.pkg).unwrap();
    assert_eq!(read_sbs(&saved, "saved.sbs", &|_| None).unwrap().pkg, report.pkg);
}
#[test]
fn imports_vector_constants_and_unary_operations() {
    let nodes = node("1", "const_float2", r#"<constValue><constantValueFloat2 v="-0.25 0.75"/></constValue>"#, "") + &node("2", "abs", "", &wire("a", "1"));
    let report = read(&xml(&nodes, "2", "transformation", "offset"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let p = materialcraft_nodes::function::compile(&report.pkg.graphs[0].nodes[0].functions["offset"], &BTreeMap::new(), 0).unwrap();
    assert_eq!(p.dims(), 2);
    assert_eq!(&p.eval([0.0; 2], [1.0; 2], &[], true, &mut Vec::new())[..2], &[0.25, 0.75]);
}
#[test]
fn imports_pow2_log2_and_cartesian_expressions() {
    let nodes = [
        node("1", "get_float1", r#"<constantValueString v="amount"/>"#, ""),
        node("2", "pow2", "", &wire("a", "1")),
        node("3", "const_float1", r#"<constantValue><constantValueFloat1 v="2"/></constantValue>"#, ""),
        node("4", "log2", "", &wire("a", "3")),
        node("5", "cartesian", "", &(wire("a", "2") + &wire("b", "4"))),
    ]
    .concat();
    let report = read(&xml(&nodes, "5", "transformation", "offset"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let f = &report.pkg.graphs[0].nodes[0].functions["offset"];
    let mut vars = BTreeMap::new();
    vars.insert("amount".into(), Value::Float(3.0));
    let p = materialcraft_nodes::function::compile(f, &vars, 0).unwrap();
    assert_eq!(p.dims(), 2);
    let result = p.eval([0.0; 2], [1.0; 2], &[], true, &mut Vec::new());
    assert!((result[0] - 8.0 * 1.0f32.cos()).abs() < 1e-6, "{result:?}");
    assert!((result[1] - 8.0 * 1.0f32.sin()).abs() < 1e-6, "{result:?}");
    assert_eq!(&result[2..], &[0.0, 0.0]);
}
#[test]
fn invalid_or_unsupported_expressions_warn_and_keep_defaults() {
    let valid = xml(&expression(), "99999999999", "blur", "intensity");
    for (case, text) in [
        valid.replace("rootnode v=\"99999999999\"", "rootnode v=\"missing\""),
        valid.replace("function v=\"sub\"", "function v=\"while\""),
        valid.replace("connRef v=\"2\"", "connRef v=\"missing\""),
        valid.replace("connRef v=\"2\"", "connRef v=\"99999999999\""),
        valid.replace("identifier v=\"b\"", "identifier v=\"a\""),
        valid.replace("identifier v=\"b\"", "identifier v=\"unknown\""),
        valid.replace("uid v=\"2\"", "uid v=\"1\""),
        valid.replace("constantValueString v=\"amount\"", "constantValueString v=\"unknown\""),
        valid.replace("get_float1", "get_float2"),
        valid.replace("function v=\"sub\"", "function v=\"sub\"/><disabled v=\"1\""),
        valid.replace("function v=\"sub\"", "function v=\"sub\"/><type v=\"16\""),
        valid.replace(&wire("b", "2"), ""),
        xml(&node("1", "const_float2", r#"<constantValueFloat2 v="1 2"/>"#, ""), "1", "blur", "intensity"),
    ]
    .into_iter()
    .enumerate()
    {
        let report = read(&text);
        assert!(report.pkg.graphs[0].nodes[0].functions.is_empty(), "case {case}: unsupported expression was imported");
        assert!(report.warnings.iter().any(|w| w.contains("imported its default")), "{:?}", report.warnings);
    }
}
#[test]
fn oversized_functions_are_rejected_and_simple_bindings_still_work() {
    let constant = node("1", "const_float1", r#"<constantValueFloat1 v="1"/>"#, "");
    let report = read(&xml(&constant.repeat(4097), "1", "blur", "intensity"));
    assert!(report.pkg.graphs[0].nodes[0].functions.is_empty());
    assert!(report.warnings.iter().any(|w| w.contains("4096")));
    let getter = node("1", "get_float1", r#"<constantValueString v="amount"/>"#, "");
    let report = read(&xml(&getter, "1", "blur", "intensity"));
    assert_eq!(report.pkg.graphs[0].nodes[0].bindings["intensity"], "amount");
    assert!(report.warnings.is_empty());
}
#[test]
fn malformed_pow2_log2_cartesian_warn_and_keep_defaults() {
    let get = node("1", "get_float1", r#"<constantValueString v="amount"/>"#, "");
    let constant = node("3", "const_float1", r#"<constantValue><constantValueFloat1 v="2"/></constantValue>"#, "");
    let constant2 = node("6", "const_float2", r#"<constValue><constantValueFloat2 v="1 2"/></constValue>"#, "");
    let constant2b = node("7", "const_float2", r#"<constValue><constantValueFloat2 v="1 2"/></constValue>"#, "");
    let pow2 = node("2", "pow2", "", &wire("a", "1"));
    let cases = [
        xml(&(get.clone() + &pow2 + &node("5", "cartesian", "", &wire("a", "2"))), "5", "transformation", "offset"),
        xml(&(get.clone() + &node("2", "pow2", "", &(wire("a", "1") + &wire("b", "1")))), "2", "blur", "intensity"),
        xml(&(constant.clone() + &node("4", "log2", "", &(wire("a", "3") + &wire("a", "3")))), "4", "blur", "intensity"),
        xml(&(get.clone() + &node("2", "pow2", "", "")), "2", "blur", "intensity"),
        xml(&(constant.clone() + &node("4", "log2", "", &wire("x", "3"))), "4", "blur", "intensity"),
        xml(&(get.clone() + &pow2 + &node("5", "cartesian", "", &(wire("a", "2") + &wire("a", "2")))), "5", "transformation", "offset"),
        xml(&(constant2.clone() + &node("2", "pow2", "", &wire("a", "6"))), "2", "blur", "intensity"),
        xml(&(constant2.clone() + &node("4", "log2", "", &wire("a", "6"))), "4", "blur", "intensity"),
        xml(&(constant2.clone() + &constant + &node("5", "cartesian", "", &(wire("a", "6") + &wire("b", "3")))), "5", "blur", "intensity"),
        xml(&(constant.clone() + &constant2 + &node("5", "cartesian", "", &(wire("a", "3") + &wire("b", "6")))), "5", "blur", "intensity"),
        xml(&(constant2.clone() + &constant2b + &node("5", "cartesian", "", &(wire("a", "6") + &wire("b", "7")))), "5", "blur", "intensity"),
        xml(
            &(constant.clone() + &node("5", "cartesian", "", &(wire("a", "3") + &wire("b", "3"))) + &node("2", "pow2", "", &wire("a", "5"))),
            "2",
            "blur",
            "intensity",
        ),
        xml(
            &(constant.clone() + &node("5", "cartesian", "", &(wire("a", "3") + &wire("b", "3"))) + &node("4", "log2", "", &wire("a", "5"))),
            "4",
            "blur",
            "intensity",
        ),
        xml(
            &(constant2.clone()
                + &node("2", "pow2", "", &wire("a", "6"))
                + &constant
                + &node("4", "log2", "", &wire("a", "3"))
                + &node("5", "cartesian", "", &(wire("a", "2") + &wire("b", "4")))),
            "5",
            "blur",
            "intensity",
        ),
        xml(
            &(constant2.clone()
                + &node("2", "pow2", "", &wire("a", "6"))
                + &constant2b
                + &node("4", "log2", "", &wire("a", "7"))
                + &node("5", "cartesian", "", &(wire("a", "2") + &wire("b", "4")))),
            "5",
            "blur",
            "intensity",
        ),
    ];
    for (case, text) in cases.into_iter().enumerate() {
        let report = read(&text);
        assert!(report.pkg.graphs[0].nodes[0].functions.is_empty(), "case {case}: malformed pow2/log2/cartesian expression was imported");
        assert!(report.warnings.iter().any(|w| w.contains("imported its default")), "case {case}: {:?}", report.warnings);
    }
}
