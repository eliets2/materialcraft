use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use materialcraft_core::{MaterialChannel, Package};
use materialcraft_image::Image;
use serde::{Deserialize, Serialize};

use crate::{MaterialSource, Session};

/// Automatic graph outputs with explicit channel overrides
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialPreview {
    pub graph: Option<usize>,
    pub channels: BTreeMap<MaterialChannel, MaterialSource>,
}

impl MaterialPreview {
    pub fn for_graph(package: &Package, graph: usize) -> Self {
        Self { graph: package.graphs.get(graph).filter(|graph| graph.function.is_none()).map(|_| graph), channels: BTreeMap::new() }
    }

    pub fn sources(&self, package: &Package) -> BTreeMap<MaterialChannel, MaterialSource> {
        let mut sources = BTreeMap::new();
        if let Some(index) = self.graph
            && let Some(graph) = package.graphs.get(index)
        {
            for node in graph.nodes.iter().filter(|node| node.def == "output") {
                let source = MaterialSource { graph: index, node: node.id, port: "output".into() };
                if let Some(channel) = MaterialChannel::from_id(materialcraft_eval::output_usage(node))
                    && source.is_image(package)
                {
                    sources.entry(channel).or_insert(source);
                }
            }
        }
        sources.extend(self.channels.clone());
        sources
    }

    pub fn graphs(&self) -> BTreeSet<usize> {
        self.graph.into_iter().chain(self.channels.values().map(|source| source.graph)).collect()
    }

    pub(crate) fn remove_graph(&mut self, graph: usize) {
        self.graph = self.graph.filter(|index| *index != graph).map(|index| if index > graph { index - 1 } else { index });
        self.channels.retain(|_, source| source.graph != graph);
        for source in self.channels.values_mut() {
            if source.graph > graph {
                source.graph -= 1;
            }
        }
    }
}

impl Session {
    /// Show a material graph on open, retaining the material while editing functions
    pub(crate) fn open_material_preview(&mut self) {
        if self.graph().is_ok_and(|graph| graph.function.is_none()) {
            self.material_preview = MaterialPreview::for_graph(&self.pkg, self.graph);
        }
    }

    pub(crate) fn clear_preview_results(&mut self) {
        self.cooked_graph = None;
        self.material_cooked.clear();
    }

    pub fn material_image(&self, source: &MaterialSource) -> Option<&Image> {
        let port = source.port_index(&self.pkg)?;
        let cooked = self.preview_cooked(source.graph)?;
        cooked.image(source.node, port)
    }

    fn preview_cooked(&self, graph: usize) -> Option<&materialcraft_eval::Cooked> {
        if self.cooked_graph == Some(graph) { Some(&self.cooked) } else { self.material_cooked.get(&graph) }
    }

    /// Assigned image channels, including sources outside the open graph
    pub fn material_maps(&self) -> Vec<(&'static str, &Image)> {
        self.material_preview.sources(&self.pkg).into_iter().filter_map(|(channel, source)| Some((channel.id(), self.material_image(&source)?))).collect()
    }

    /// A source's identity, selected port and cooked content
    pub fn material_source_key(&self, source: &MaterialSource) -> u64 {
        let result = self.preview_cooked(source.graph).and_then(|cooked| cooked.nodes.get(&source.node));
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        (source, source.port_index(&self.pkg), result.map(|result| result.key)).hash(&mut hash);
        hash.finish()
    }

    /// Assignment and cooked content both invalidate the rendered preview
    pub fn material_key(&self) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for (channel, source) in self.material_preview.sources(&self.pkg) {
            (channel, self.material_source_key(&source)).hash(&mut hash);
        }
        hash.finish()
    }
}
