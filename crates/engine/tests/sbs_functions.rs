//! Imported numeric parameter expressions must affect real cooks and follow exposed inputs.
use materialcraft_engine::{execute, formats::sbs, new_session};
use serde_json::json;

#[test]
fn imported_expression_cooks_follows_inputs_and_survives_undo_and_save() {
    // Original structural XML: in_low = amount - 0.25, with no embedded native package.
    let xml = r#"<package><content><graph><identifier v="Math"/>
      <paraminputs><paraminput><identifier v="amount"/><uid v="90"/><type v="256"/>
        <defaultValue><constantValueFloat1 v="0.75"/></defaultValue></paraminput></paraminputs>
      <compNodes><compNode><uid v="10"/><compImplementation><compFilter><filter v="levels"/>
        <parameters><parameter><name v="levelinlow"/><paramValue><dynamicValue><rootnode v="4"/>
          <paramNodes>
            <paramNode><uid v="1"/><function v="get_float1"/><funcDatas><funcData><name v="get_float1"/>
              <constantValueString v="amount"/></funcData></funcDatas></paramNode>
            <paramNode><uid v="2"/><function v="const_float1"/><funcDatas><funcData><name v="const_float1"/>
              <constantValueFloat1 v="0.25"/></funcData></funcDatas></paramNode>
            <paramNode><uid v="3"/><function v="sub"/><connections>
              <connection><identifier v="b"/><connRef v="2"/></connection>
              <connection><identifier v="a"/><connRef v="1"/></connection>
            </connections></paramNode>
          <paramNode><uid v="4"/><function v="swizzle4"/><connections><connection><identifier v="vector"/><connRef v="3"/></connection></connections></paramNode></paramNodes></dynamicValue></paramValue></parameter></parameters>
      </compFilter></compImplementation></compNode></compNodes></graph></content></package>"#;
    let imported = sbs::read_sbs(xml.as_bytes(), "math.sbs", &|_| None).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    let mut s = new_session();
    s.preview_size = Some(64);
    s.pkg = imported.pkg;
    let levels = s.graph().unwrap().nodes[0].id;
    let uniform = execute(&mut s, "graph.node.add", &json!({"def": "uniform", "params": {"color_mode": false}})).unwrap()["id"].as_u64().unwrap();
    execute(&mut s, "graph.connect", &json!({"from": uniform, "to": levels.0})).unwrap();
    let mean = |s: &mut materialcraft_engine::Session| {
        let levels = s.graph().unwrap().nodes.iter().find(|node| node.def == "levels").unwrap().id;
        s.cook();
        assert!(s.cooked.nodes[&levels].error.is_none());
        s.cooked.image(levels, 0).unwrap().mean()[0]
    };
    assert!(mean(&mut s).abs() < 1e-5);
    execute(&mut s, "graph.exposed.set", &json!({"id": "amount", "value": 0.5})).unwrap();
    assert!((mean(&mut s) - 1.0 / 3.0).abs() < 1e-5);
    execute(&mut s, "edit.undo", &json!({})).unwrap();
    assert!(mean(&mut s).abs() < 1e-5);
    execute(&mut s, "edit.redo", &json!({})).unwrap();
    assert!((mean(&mut s) - 1.0 / 3.0).abs() < 1e-5);
    let saved = sbs::write_sbs(&s.pkg).unwrap();
    s.pkg = sbs::read_sbs(&saved, "saved.sbs", &|_| None).unwrap().pkg;
    s.cache.lock().unwrap().clear();
    assert!((mean(&mut s) - 1.0 / 3.0).abs() < 1e-5);
    assert!(s.graph().unwrap().nodes.iter().find(|node| node.def == "levels").unwrap().functions.contains_key("in_low"));
}
