pub(crate) mod actions;
pub(crate) mod add_node_menu;
pub(crate) mod annotation_canvas;
pub(crate) mod annotation_input;
pub(crate) mod canvas;
mod context_menu;
pub(crate) mod frame_hit;
#[cfg(test)]
mod frames;
mod graph_camera;
mod graph_drag;
pub(crate) mod graph_parent;
mod graph_view;
pub(crate) mod input_visibility;
#[cfg(test)]
mod interaction_tests;
mod library_drag;
pub(crate) mod node_card;
mod node_layout;
mod node_search;
pub(crate) mod node_selection;
pub(crate) mod port_hit;
pub(crate) mod port_label;
pub(crate) mod subgraph_navigation;

pub use canvas::show;
pub(crate) use graph_camera::GraphCamera;
use graph_drag::GraphDrag;
pub use graph_view::GraphView;
pub use library_drag::LibraryDrag;
pub use navigation::add_node_near_selection;
use node_search::NodeSearch;
pub use node_style::{NODE_W, header_color, node_label, node_rect, port_color};
pub use port_hit::PortHit;
pub use wire_actions::{delete_selected_wires, swap_selected_wires};
pub type WireRef = (materialcraft_engine::core::NodeId, String);

#[cfg(test)]
mod canvas_tests;
pub(crate) mod grid;
mod input;
mod navigation;
mod node_style;
mod search;
pub(crate) mod selection_commands;
pub(crate) mod toolbar;
mod wire_actions;
pub(crate) mod wire_geometry;

#[cfg(test)]
mod review_tests;

pub(crate) mod edit_menu;
pub(crate) mod editor_actions;
pub(crate) mod editor_target;
pub(crate) mod gesture;

pub(crate) mod canvas_geometry;

pub use navigation::import_bitmap_bytes;
#[cfg(not(target_arch = "wasm32"))]
pub use navigation::import_bitmap_file;
