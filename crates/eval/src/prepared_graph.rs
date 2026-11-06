use crate::{Port, ports_with_connections};
use materialcraft_core::{Graph, GraphIndex, NodeId, Package};
use std::collections::BTreeMap;

/// Port definitions resolved once for a cook, alongside borrowed graph lookups
pub(crate) struct PreparedGraph<'a> {
    pub index: GraphIndex<'a>,
    pub order: Vec<NodeId>,
    nominal_sizes: BTreeMap<NodeId, (u32, u32)>,
    ports: BTreeMap<NodeId, (Vec<Port>, Vec<Port>)>,
}

impl<'a> PreparedGraph<'a> {
    pub fn new(package: &Package, graph: &'a Graph, roots: Option<&[NodeId]>) -> Self {
        let index = GraphIndex::new(graph);
        let needed = roots.map(|roots| index.required_nodes(roots));
        let order: Vec<_> = index.topo_order().into_iter().filter(|id| needed.as_ref().is_none_or(|needed| needed.contains(id))).collect();
        let ports = order
            .iter()
            .filter_map(|id| {
                let node = index.node(*id)?;
                Some((*id, ports_with_connections(package, node, index.incoming(*id).iter().copied())))
            })
            .collect();

        let mut prepared = Self { index, order, ports, nominal_sizes: BTreeMap::new() };
        for &id in &prepared.order {
            let Some(node) = prepared.index.node(id) else { continue };

            let input = prepared.first_input_size(id);
            let nominal = if node.disabled {
                input.unwrap_or_else(|| crate::node_size(graph, materialcraft_nodes::find(&node.def), node, None, None))
            } else {
                crate::node_size(graph, materialcraft_nodes::find(&node.def), node, input, None)
            };
            prepared.nominal_sizes.insert(id, nominal);
        }
        prepared
    }

    pub fn first_input_size(&self, id: NodeId) -> Option<(u32, u32)> {
        self.ports(id).0.iter().filter(|port| port.kind != materialcraft_nodes::PortKind::Value).find_map(|port| {
            let connection = self.index.input_of(id, &port.id)?;
            self.nominal_sizes.get(&connection.from).copied()
        })
    }

    pub fn ports(&self, id: NodeId) -> (&[Port], &[Port]) {
        self.ports.get(&id).map_or((&[], &[]), |(inputs, outputs)| (inputs.as_slice(), outputs.as_slice()))
    }
}
