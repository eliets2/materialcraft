use super::embedded::{base64_engine, decode_embedded, encode_embedded};
use super::read::{is_sbs, read_sbs};
use super::write::write_sbs;
use base64::Engine as _;
use materialcraft_core::{Frame, Graph, MAX_SIZE_LOG2, MIN_SIZE_LOG2, MISSING_NODE, Package, Resource, ResourceBytes, SizeMode, Value, missing_ports};

fn no_linked_files(_: &str) -> Option<Vec<u8>> {
    None
}

fn sample_package() -> Package {
    let mut pkg = Package::new("Primitives");
    let mut graph = Graph::new("Wall");
    graph.size = [9, 9];

    let curve = graph.add_node("curve", [0.0, 0.0]);
    let blur = graph.add_node("blur", [150.0, 0.0]);
    let uniform = graph.add_node("uniform", [0.0, 150.0]);
    let blend = graph.add_node("blend", [300.0, 0.0]);
    let output = graph.add_node("output", [450.0, 0.0]);
    graph.connect(curve, "output", blur, "input").unwrap();
    graph.connect(blur, "output", blend, "foreground").unwrap();
    graph.connect(uniform, "output", blend, "background").unwrap();
    graph.connect(blend, "output", output, "input").unwrap();

    let blur_node = graph.node_mut(blur).unwrap();
    blur_node.params.insert("intensity".into(), Value::Float(4.5));
    blur_node.base.size_mode = SizeMode::Absolute;
    blur_node.base.size = [8, 8];
    graph.node_mut(uniform).unwrap().params.insert("color".into(), Value::Color([0.25, 0.5, 0.75, 1.0]));
    graph.node_mut(blend).unwrap().params.insert("mode".into(), Value::Enum(3));
    let output_node = graph.node_mut(output).unwrap();
    output_node.params.insert("usage".into(), Value::Enum(4));
    output_node.params.insert("identifier".into(), Value::Text("height".into()));
    graph.node_mut(curve).unwrap().params.insert("curve".into(), Value::Curve(materialcraft_core::Curve::default()));
    graph.frames.push(Frame {
        id: 90,
        title: "Frame & <title>".into(),
        title_visible: false,
        description: "note".into(),
        rect: [-10.0, -10.0, 300.0, 200.0],
        color: [0.1, 0.2, 0.3, 0.4],
    });
    pkg.graphs.push(graph);

    let orange = materialcraft_image::Image::filled(4, 4, materialcraft_image::Kind::Color, [1.0, 0.5, 0.0, 1.0]);
    let png = crate::encode(&orange, "png", crate::Depth::U8, true).unwrap();
    pkg.resources.push(Resource::new("tex", "tex.png", ResourceBytes::new(png)));
    pkg
}

#[test]
fn sniffs_sbs() {
    assert!(is_sbs(b"<?xml version=\"1.0\"?>\n<package><content/></package>"));
    assert!(is_sbs(b"\xef\xbb\xbf<package>"));
    assert!(is_sbs(b"<!-- hi --><package >"));
    assert!(!is_sbs(b"<packages>"));
    assert!(!is_sbs(b"PK\x03\x04zip"));
    assert!(!is_sbs(b"<?xml"));
    assert!(!is_sbs(b""));
}

#[test]
fn structural_round_trip() {
    let pkg = sample_package();
    let plain = String::from_utf8(write_sbs(&pkg).unwrap()).unwrap();
    assert!(!plain.contains("materialcraft-package"));
    assert!(!plain.contains("materialcraft://"));
    let read = read_sbs(plain.as_bytes(), "x.sbs", &no_linked_files).unwrap();

    let (original, back) = (&pkg.graphs[0], &read.pkg.graphs[0]);
    assert_eq!(back.name, "Wall");
    assert_eq!(back.size, [9, 9]);
    assert_eq!(back.nodes.len(), original.nodes.len());
    assert_eq!(back.connections.len(), original.connections.len());
    for (a, b) in original.nodes.iter().zip(&back.nodes) {
        assert_eq!(a.def, b.def);
        assert!((a.pos[0] - b.pos[0]).abs() < 1e-3 && (a.pos[1] - b.pos[1]).abs() < 1e-3);
        assert_eq!(a.base, b.base);
    }

    assert_eq!(back.nodes[1].params.get("intensity"), Some(&Value::Float(4.5)));
    let Some(Value::Color(color)) = back.nodes[2].params.get("color") else { panic!("uniform color") };

    assert!((color[0] - 0.25).abs() < 1e-3 && (color[2] - 0.75).abs() < 1e-3);
    assert_eq!(back.nodes[3].params.get("mode"), Some(&Value::Enum(3)));
    assert_eq!(back.nodes[4].params.get("usage"), Some(&Value::Enum(4)));
    assert_eq!(back.nodes[4].params.get("identifier"), Some(&Value::Text("height".into())));
    assert_eq!(back.frames[0].title, "Frame & <title>");
    assert!(!back.frames[0].title_visible);
    assert_eq!(read.pkg.resources[0].bytes, pkg.resources[0].bytes);

    let blend = back.nodes[3].id;
    assert_eq!(back.input_of(blend, "foreground").map(|c| c.from), Some(back.nodes[1].id));
    assert_eq!(back.input_of(blend, "background").map(|c| c.from), Some(back.nodes[2].id));
}

#[test]
fn native_data_is_the_saved_document() {
    let text = String::from_utf8(write_sbs(&sample_package()).unwrap()).unwrap().replace("<identifier v=\"Wall\"/>", "<identifier v=\"Wall2\"/>");
    let read = read_sbs(text.as_bytes(), "x.sbs", &no_linked_files).unwrap();

    assert_eq!(read.pkg.graphs[0].name, "Wall2");
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
}

#[test]
fn missing_nodes_are_placeholders_that_survive_saving() {
    let xml = r#"<?xml version="1.0"?><package><identifier v="P"/><dependencies><dependency><filename v="sbs://noise_fancy.sbs"/><uid v="7"/></dependency></dependencies><content><graph><identifier v="G"/>
        <compNodes>
          <compNode><uid v="1"/><GUILayout><gpos v="0 0 0"/></GUILayout><compOutputs><compOutput><uid v="11"/><comptype v="2"/></compOutput></compOutputs>
            <compImplementation><compInstance><path v="pkg:///fancy_noise?dependency=7"/><parameters><parameter><name v="scale"/><paramValue><constantValueFloat1 v="3"/></paramValue></parameter></parameters>
            <outputBridgings><outputBridging><uid v="11"/><identifier v="noise"/></outputBridging></outputBridgings></compInstance></compImplementation></compNode>
          <compNode><uid v="2"/><GUILayout><gpos v="200 0 0"/></GUILayout><compOutputs><compOutput><uid v="21"/><comptype v="1"/></compOutput></compOutputs>
            <connections><connection><identifier v="input1"/><connRef v="1"/><connRefOutput v="11"/></connection></connections>
            <compImplementation><compFilter><filter v="blur"/><parameters><parameter><name v="intensity"/><paramValue><constantValueFloat1 v="6"/></paramValue></parameter><parameter><name v="bogus"/><paramValue><constantValueFloat1 v="6"/></paramValue></parameter></parameters></compFilter></compImplementation></compNode>
          <compNode><uid v="3"/><GUILayout><gpos v="400 0 0"/></GUILayout><compOutputs><compOutput><uid v="31"/><comptype v="1"/></compOutput></compOutputs>
            <connections><connection><identifier v="weird_in"/><connRef v="2"/><connRefOutput v="21"/></connection></connections>
            <compImplementation><compFilter><filter v="futurefilter"/><parameters/></compFilter></compImplementation></compNode>
        </compNodes></graph></content></package>"#;
    let read = read_sbs(xml.as_bytes(), "p.sbs", &no_linked_files).unwrap();
    let graph = &read.pkg.graphs[0];
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.connections.len(), 2);

    let fancy = &graph.nodes[0];
    assert_eq!(fancy.def, MISSING_NODE);
    assert_eq!(fancy.label, "fancy_noise");
    assert_eq!(missing_ports(fancy).1, vec![("noise".to_string(), true)]);
    assert_eq!(graph.nodes[1].params.get("intensity"), Some(&Value::Float(6.0)));
    assert_eq!(graph.input_of(graph.nodes[1].id, "input").map(|c| c.from_port.as_str()), Some("noise"));
    assert_eq!(missing_ports(&graph.nodes[2]).0, vec!["weird_in".to_string()]);
    for expected in ["missing node `fancy_noise`", "missing node `futurefilter`", "bogus"] {
        assert!(read.warnings.iter().any(|w| w.contains(expected)), "{expected}: {:?}", read.warnings);
    }

    // Saved and read back from the SBS data alone, the placeholders are written as they came
    let text = String::from_utf8(write_sbs(&read.pkg).unwrap()).unwrap();
    assert!(text.contains("sbs://noise_fancy.sbs"));
    assert!(text.contains("pkg:///fancy_noise?dependency="));
    assert!(text.contains("<filter v=\"futurefilter\"/>"));
    let back = read_sbs(text.as_bytes(), "p.sbs", &no_linked_files).unwrap();
    let back_graph = &back.pkg.graphs[0];
    assert_eq!(back_graph.nodes[0].def, MISSING_NODE);
    assert_eq!(back_graph.nodes[0].params.get("sbs_parameters"), fancy.params.get("sbs_parameters"));
    assert_eq!(back_graph.connections.len(), 2);
}

#[test]
fn wires_keep_the_last_per_input_and_drop_cycles() {
    let node = |uid: u32, wires: &str| {
        format!(
            "<compNode><uid v=\"{uid}\"/><compOutputs><compOutput><uid v=\"{}\"/></compOutput></compOutputs><connections>{wires}</connections><compImplementation><compFilter><filter v=\"blend\"/></compFilter></compImplementation></compNode>",
            uid + 100
        )
    };

    let wire =
        |input: &str, from: u32| format!("<connection><identifier v=\"{input}\"/><connRef v=\"{from}\"/><connRefOutput v=\"{}\"/></connection>", from + 100);
    let xml = format!(
        "<package><content><graph><identifier v=\"G\"/><compNodes>{}{}{}{}</compNodes></graph></content></package>",
        node(1, ""),
        node(2, &format!("{}{}", wire("source", 1), wire("source", 4))),
        node(3, &wire("source", 2)),
        node(4, &wire("source", 3))
    );
    let read = read_sbs(xml.as_bytes(), "x.sbs", &no_linked_files).unwrap();
    let graph = &read.pkg.graphs[0];

    // 2 takes 4 (the last wire), which closes 2 → 3 → 4 → 2, so one wire of that loop is dropped
    assert_eq!(graph.connections.len(), 2);
    assert!(graph.input_of(graph.nodes[1].id, "foreground").is_none_or(|c| c.from == graph.nodes[3].id));
    assert_eq!(graph.topo_order().len(), 4);
    assert!(read.warnings.iter().any(|w| w.contains("cycle")), "{:?}", read.warnings);
}

#[test]
fn hostile_input_is_an_error_or_a_warning_never_a_panic() {
    let cases: &[&str] = &[
        "",
        "<package",
        "<notpackage/>",
        "<!DOCTYPE package [<!ENTITY x \"y\">]><package/>",
        "<package><content><graph/></content></package>",
        "<package><content><graph><compNodes><compNode/><compNode><uid v=\"1\"/><compImplementation><compInstance/></compImplementation></compNode><compNode><uid v=\"1\"/></compNode></compNodes></graph></content></package>",
        "<package><content><graph><options><option><name v=\"defaultParentSize\"/><value v=\"99999x-4\"/></option></options><compNodes><compNode><uid v=\"1\"/><connections><connection><identifier v=\"input1\"/><connRef v=\"1\"/><connRefOutput v=\"9\"/></connection></connections><compImplementation><compFilter><filter v=\"blur\"/><parameters><parameter><name v=\"outputsize\"/><relativeTo v=\"0\"/><paramValue><constantValueInt2 v=\"99999999999 nan\"/></paramValue></parameter><parameter><name v=\"intensity\"/><paramValue><constantValueFloat1 v=\"inf\"/></paramValue></parameter></parameters></compFilter></compImplementation></compNode></compNodes></graph></content></package>",
        "<package><content><resource><identifier v=\"r\"/><source><binembedded><strdata v=\"////\"/></binembedded></source></resource><resource><identifier v=\"s\"/><source><binembedded><strdata v=\"f////w==\"/></binembedded></source></resource><resource><identifier v=\"t\"/><filepath v=\"../../etc/passwd\"/></resource></content></package>",
        "<package><content/><!-- materialcraft-package version=1 hash=0 data=AAAA --></package>",
        "<package><content><group><content><group><content><graph><identifier v=\"Deep\"/></graph></content></group></content></group><function><identifier v=\"f\"/></function><weird/></content></package>",
    ];
    for case in cases {
        if let Ok(read) = read_sbs(case.as_bytes(), "x.sbs", &no_linked_files) {
            assert!(!read.pkg.graphs.is_empty(), "{case}");
        }
    }
    assert!(read_sbs(b"\xff\xfe", "x.sbs", &no_linked_files).is_err());

    let nested = read_sbs(cases[9].as_bytes(), "x.sbs", &no_linked_files).unwrap();
    assert!(nested.pkg.graphs.iter().any(|graph| graph.name == "Deep"));
    assert!(nested.pkg.graphs.iter().any(|graph| graph.name == "f" && graph.function.is_some()));
    assert!(nested.warnings.iter().any(|warning| warning.contains("missing function body")));
    let clamped = read_sbs(cases[6].as_bytes(), "x.sbs", &no_linked_files).unwrap();
    assert_eq!(clamped.pkg.graphs[0].size, [MAX_SIZE_LOG2, MIN_SIZE_LOG2]);
    assert!(clamped.pkg.graphs[0].connections.is_empty(), "a self-wire is a cycle");
}

#[test]
fn linked_resources_load_through_the_callback() {
    let xml = "<package><content><resource><identifier v=\"img\"/><type v=\"bitmap\"/><format v=\"png\"/><source><externalcopy><filename v=\"p.resources/img.png\"/></externalcopy></source></resource></content></package>";

    let found = read_sbs(xml.as_bytes(), "p.sbs", &|path| (path == "p.resources/img.png").then(|| vec![1, 2, 3])).unwrap();
    assert_eq!(found.pkg.resources[0].bytes.as_slice(), &[1, 2, 3]);

    let not_found = read_sbs(xml.as_bytes(), "p.sbs", &no_linked_files).unwrap();
    assert!(not_found.pkg.resources.is_empty());
    assert!(not_found.warnings.iter().any(|w| w.contains("img")), "{:?}", not_found.warnings);
}

#[test]
fn embedded_data_round_trips_and_checks_its_length() {
    let data: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
    let encoded = encode_embedded(&data).unwrap();
    assert_eq!(decode_embedded(&encoded, super::constant::MAX_EMBEDDED_BYTES).unwrap(), data);

    let mut raw = base64_engine().decode(&encoded).unwrap();
    raw[3] ^= 1;
    assert!(decode_embedded(&base64_engine().encode(raw), super::constant::MAX_EMBEDDED_BYTES).unwrap_err().contains("declared"));
}

#[test]
fn native_function_fixture_retains_operation_types_and_ports() {
    let xml = r#"<package><content><graph><identifier v="Values"/><compNodes><compNode><uid v="10"/>
      <compOutputs><compOutput><uid v="11"/><comptype v="16"/></compOutput></compOutputs>
      <compImplementation><compFilter><filter v="valueprocessor"/><parameters><parameter><name v="function"/><paramValue><dynamicValue>
        <rootnode v="22"/><paramNodes>
          <paramNode><uid v="20"/><function v="const_int1"/><type v="16"/><funcDatas><funcData><name v="const_int1"/><constantValue><constantValueInt32 v="16777217"/></constantValue></funcData></funcDatas></paramNode>
          <paramNode><uid v="21"/><function v="const_int1"/><type v="16"/><funcDatas><funcData><name v="const_int1"/><constantValue><constantValueInt32 v="3"/></constantValue></funcData></funcDatas></paramNode>
          <paramNode><uid v="22"/><function v="add"/><type v="16"/><connections><connection><identifier v="a"/><connRef v="20"/></connection><connection><identifier v="b"/><connRef v="21"/></connection></connections></paramNode>
        </paramNodes>
      </dynamicValue></paramValue></parameter></parameters></compFilter></compImplementation>
    </compNode></compNodes></graph></content></package>"#;
    let read = read_sbs(xml.as_bytes(), "values.sbs", &no_linked_files).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let back = read_sbs(&write_sbs(&read.pkg).unwrap(), "values.sbs", &no_linked_files).unwrap();
    let Value::Function(function) = &back.pkg.graphs[0].nodes[0].params["function"] else { panic!("missing function") };

    assert_eq!(function.nodes[0].value, Some(Value::Int(16_777_217)));
    assert_eq!(function.nodes[2].op, "add");
    assert_eq!(function.nodes[2].type_code, 16);
    assert_eq!(function.nodes[2].inputs, vec![Some(function.nodes[0].id), Some(function.nodes[1].id)]);
}

#[test]
fn image_output_types_follow_color_mode_and_blended_inputs() {
    let mut package = Package::new("Channels");
    let mut graph = Graph::new("Channels");
    let gray = graph.add_node("uniform", [0.0; 2]);
    graph.node_mut(gray).unwrap().params.insert("color_mode".into(), Value::Bool(false));
    let color = graph.add_node("uniform", [0.0; 2]);
    let blend = graph.add_node("blend", [0.0; 2]);
    let output = graph.add_node("output", [0.0; 2]);
    graph.connect(gray, "output", blend, "foreground").unwrap();
    graph.connect(color, "output", blend, "background").unwrap();
    graph.connect(blend, "output", output, "input").unwrap();
    package.graphs.push(graph);
    let bytes = write_sbs(&package).unwrap();
    let document = roxmltree::Document::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let types: Vec<_> = document.descendants().filter(|node| node.has_tag_name("comptype")).map(|node| node.attribute("v").unwrap()).collect();
    assert_eq!(types, ["2", "1", "1"]);
    assert_eq!(document.descendants().find(|node| node.has_tag_name("channels")).unwrap().attribute("v"), Some("1"));
}

#[test]
fn navigation_pins_round_trip_in_graphs_and_functions() {
    let canvas = r#"<GUIObjects>
        <GUIObject><type v="PIN"/><GUILayout><gpos v="-128 64 0"/></GUILayout><GUIName v="Start &amp; &lt;review&gt;"/><uid v="50"/></GUIObject>
        <GUIObject><type v="PIN"/><GUILayout><gpos v="512 -96 0"/></GUILayout><GUIName v="Finish"/><uid v="51"/></GUIObject>
    </GUIObjects>"#;
    let items = [
        format!("<graph><identifier v=\"Pins\"/>{canvas}</graph>"),
        format!("<function><identifier v=\"Pins\"/><paramValue><dynamicValue><paramNodes/>{canvas}</dynamicValue></paramValue></function>"),
    ];

    for item in items {
        let xml = format!("<package><content>{item}</content></package>");
        let imported = read_sbs(xml.as_bytes(), "pins.sbs", &no_linked_files).unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        let pins = &imported.pkg.graphs[0].pins;
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].name, "Start & <review>");
        assert_eq!(pins[0].pos, [-128.0, 64.0]);
        assert_eq!(pins[1].name, "Finish");
        assert_eq!(pins[1].pos, [512.0, -96.0]);
        assert_ne!(pins[0].id, pins[1].id);

        let saved = write_sbs(&imported.pkg).unwrap();
        let native = roxmltree::Document::parse(std::str::from_utf8(&saved).unwrap()).unwrap();
        let pin_count = native.descendants().filter(|element| element.has_tag_name("type") && element.attribute("v") == Some("PIN")).count();
        assert_eq!(pin_count, 2);

        let reopened = read_sbs(&saved, "pins.sbs", &no_linked_files).unwrap();
        assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
        assert_eq!(reopened.pkg.graphs[0].pins, *pins);
    }
}

#[test]
fn component_usages_round_trip_without_reassigning_existing_indices() {
    use materialcraft_core::ComponentUsage;

    let original = ["baseColor", "normal", "roughness", "metallic", "height", "ambientOcclusion", "emissive", "opacity", "specular", "any"];
    for (index, id) in original.iter().enumerate() {
        assert_eq!(ComponentUsage::from_index(index as u32).id, *id);
    }
    assert_eq!(ComponentUsage::from_index(u32::MAX), ComponentUsage::ANY);
    assert_eq!(ComponentUsage::index("futureUsage"), ComponentUsage::ANY_INDEX);
    assert_eq!(ComponentUsage::index(" DIFFUSE "), ComponentUsage::index("diffuse"));

    let mut package = Package::new("Usage round trip");
    let mut graph = Graph::new("Usages");
    for definition in ["output", "input_color", "input_grayscale", "input_value"] {
        for (index, usage) in ComponentUsage::ALL.iter().enumerate() {
            let id = graph.add_node(definition, [index as f32 * 160.0, 0.0]);
            graph
                .node_mut(id)
                .unwrap()
                .params
                .extend([("identifier".into(), Value::Text(format!("{definition}_{}", usage.id))), ("usage".into(), Value::Enum(index as u32))]);
        }
    }
    package.graphs.push(graph);
    let xml = write_sbs(&package).unwrap();
    let restored = read_sbs(&xml, "usages.sbs", &no_linked_files).unwrap();
    assert!(restored.warnings.is_empty(), "{:?}", restored.warnings);
    assert_eq!(restored.pkg.graphs[0].nodes.len(), package.graphs[0].nodes.len());
    for (original, node) in package.graphs[0].nodes.iter().zip(&restored.pkg.graphs[0].nodes) {
        assert_eq!(node.def, original.def);
        assert_eq!(node.params.get("usage"), original.params.get("usage"));
        assert_eq!(node.params.get("identifier"), original.params.get("identifier"));
    }
}

#[test]
fn input_usage_defaults_and_unknown_metadata_stay_unclassified() {
    use materialcraft_core::ComponentUsage;

    let mut package = Package::new("Input usages");
    let mut graph = Graph::new("Inputs");
    for definition in ["input_color", "input_grayscale", "input_value"] {
        graph.add_node(definition, [0.0; 2]);
    }
    package.graphs.push(graph);
    let saved = String::from_utf8(write_sbs(&package).unwrap()).unwrap();
    let usage = "<usages><usage><components v=\"RGBA\"/><name v=\"any\"/></usage></usages>";
    assert_eq!(saved.matches(usage).count(), 3);
    for xml in [saved.clone(), saved.replace(usage, ""), saved.replace("<name v=\"any\"/>", "<name v=\"futureUsage\"/>")] {
        let restored = read_sbs(xml.as_bytes(), "inputs.sbs", &no_linked_files).unwrap();
        assert_eq!(restored.pkg.graphs[0].nodes.len(), 3);
        for node in &restored.pkg.graphs[0].nodes {
            assert_eq!(node.params.get("usage"), Some(&Value::Enum(ComponentUsage::ANY_INDEX)));
        }
    }
}
