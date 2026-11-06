use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use materialcraft_core::Package;
use materialcraft_eval::{Cache, Cooked, Options};

/// One background result for the editor and the assigned material sources
#[derive(Default)]
pub struct PreviewCook {
    pub graph: usize,
    pub cooked: Cooked,
    pub materials: BTreeMap<usize, Cooked>,
}

impl PreviewCook {
    pub fn run(package: &Package, graph: usize, materials: &BTreeSet<usize>, options: &Options<'_>, cache: &Arc<Mutex<Cache>>) -> Self {
        let mut result = Self { graph, cooked: materialcraft_eval::cook(package, graph, options, cache), materials: BTreeMap::new() };
        for &index in materials {
            if result.cancelled() {
                break;
            }
            if index != graph && package.graphs.get(index).is_some() {
                result.materials.insert(index, materialcraft_eval::cook(package, index, options, cache));
            }
        }
        result
    }

    pub fn cancelled(&self) -> bool {
        self.cooked.cancelled || self.materials.values().any(|cooked| cooked.cancelled)
    }
}
