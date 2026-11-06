use crate::Resource;
use crate::{CoreError, Graph, Result};
use serde::{Deserialize, Serialize};

pub const PACKAGE_EXT: &str = "sbs";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub graphs: Vec<Graph>,
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Format version of `package.json`
    #[serde(default = "version")]
    pub version: u32,
}

fn version() -> u32 {
    1
}

impl Package {
    pub fn new(name: &str) -> Package {
        Package { name: name.into(), graphs: Vec::new(), resources: Vec::new(), version: 1 }
    }

    pub fn graph(&self, i: usize) -> Result<&Graph> {
        self.graphs.get(i).ok_or(CoreError::NoGraph(i))
    }

    pub fn graph_mut(&mut self, i: usize) -> Result<&mut Graph> {
        self.graphs.get_mut(i).ok_or(CoreError::NoGraph(i))
    }

    pub fn graph_by_name(&self, name: &str) -> Option<(usize, &Graph)> {
        self.graphs.iter().enumerate().find(|(_, g)| g.name == name)
    }

    pub fn resource(&self, name: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.name == name)
    }

    /// A name not used by any graph: `base`, `base_1`, `base_2`…
    pub fn unique_graph_name(&self, base: &str) -> String {
        let base = if base.trim().is_empty() { "Graph" } else { base.trim() };
        if self.graph_by_name(base).is_none() {
            return base.to_string();
        }
        (1..).map(|i| format!("{base}_{i}")).find(|n| self.graph_by_name(n).is_none()).unwrap_or_else(|| base.to_string())
    }
}
