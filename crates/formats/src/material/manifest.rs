use super::{ImageInput, MapEntry, Parameter, PresetEntry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub name: String,
    /// The graph of the source package the maps were cooked from
    pub graph: String,
    /// Size of the maps in pixels
    pub size: [u32; 2],
    pub maps: Vec<MapEntry>,
    /// Parameters other apps may change (scalar, vector, color, toggle and option inputs)
    pub parameters: Vec<Parameter>,
    /// Image inputs of the graph, fed from mesh maps by identifier
    pub inputs: Vec<ImageInput>,
    #[serde(default)]
    pub presets: Vec<PresetEntry>,
    pub source: String,
}
