use super::{
    constant::{FIRST_UID, FORMAT_VERSION, SELF_DEPENDENCY},
    embedded::encode_embedded,
    sbs_writer::SbsWriter,
    xml::xml_escape,
};
use crate::Result;
use materialcraft_core::Package;
use std::{collections::BTreeMap, fmt::Write as _};

pub fn write_sbs(pkg: &Package) -> Result<Vec<u8>> {
    let mut writer = SbsWriter { last_uid: FIRST_UID, external_packages: BTreeMap::new() };
    let self_uid = writer.next_uid();

    let mut content = String::from("<content>");
    for graph in &pkg.graphs {
        if graph.function.is_some() {
            super::standalone_function::write(&mut content, graph, &mut writer)?;
        } else {
            writer.write_graph(&mut content, pkg, graph, self_uid)?;
        }
    }
    if !pkg.resources.is_empty() {
        let _ = write!(content, "<group><uid v=\"{}\"/><identifier v=\"Resources\"/><content>", writer.next_uid());
        for resource in &pkg.resources {
            let svg = resource.file.to_ascii_lowercase().ends_with(".svg");
            let ext = resource.file.rsplit_once('.').map_or("png", |(_, e)| e);
            let _ = write!(
                content,
                "<resource><identifier v=\"{}\"/><uid v=\"{}\"/><type v=\"{}\"/><format v=\"{}\"/><filepath v=\"\"/><source><binembedded><datalength v=\"{}\"/><strdata v=\"{}\"/></binembedded></source></resource>",
                xml_escape(&resource.name),
                writer.next_uid(),
                if svg { "svg" } else { "bitmap" },
                xml_escape(ext),
                resource.bytes.len(),
                encode_embedded(resource.bytes.as_slice())?
            );
        }
        content.push_str("</content></group>");
    }
    content.push_str("</content>");

    let mut out = String::with_capacity(content.len() + 4096);
    let _ = write!(
        out,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><package><identifier v=\"{}\"/><formatVersion v=\"{FORMAT_VERSION}\"/><updaterVersion v=\"{FORMAT_VERSION}\"/><fileUID v=\"0\"/><versionUID v=\"0\"/><dependencies>",
        xml_escape(&pkg.name)
    );
    let write_dependency = |out: &mut String, file: &str, uid: u64| {
        let _ = write!(
            out,
            "<dependency><filename v=\"{}\"/><uid v=\"{uid}\"/><type v=\"package\"/><fileUID v=\"0\"/><versionUID v=\"0\"/></dependency>",
            xml_escape(file)
        );
    };

    write_dependency(&mut out, SELF_DEPENDENCY, self_uid);
    for (file, uid) in &writer.external_packages {
        write_dependency(&mut out, file, *uid);
    }
    out.push_str("</dependencies>");
    out.push_str(&content);

    out.push_str("</package>\n");
    Ok(out.into_bytes())
}
