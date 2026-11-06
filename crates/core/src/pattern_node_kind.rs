use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatternNodeKind {
    #[serde(rename = "paramset")]
    Quadrant,
    #[serde(rename = "markov2")]
    Switch,
    #[serde(rename = "addnode")]
    Iterate,
    #[serde(rename = "passthrough")]
    Dot,
}

impl PatternNodeKind {
    pub const ALL: [Self; 4] = [Self::Quadrant, Self::Switch, Self::Iterate, Self::Dot];

    pub fn native_id(self) -> &'static str {
        match self {
            Self::Quadrant => "paramset",
            Self::Switch => "markov2",
            Self::Iterate => "addnode",
            Self::Dot => "passthrough",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quadrant => "Quadrant",
            Self::Switch => "Switch",
            Self::Iterate => "Iterate",
            Self::Dot => "Dot",
        }
    }

    pub fn child_count(self) -> usize {
        match self {
            Self::Quadrant => 4,
            Self::Switch | Self::Iterate => 2,
            Self::Dot => 1,
        }
    }

    pub fn child_port(self, slot: usize) -> Option<&'static str> {
        if self == Self::Dot {
            return (slot == 0).then_some("output");
        }
        ["output0", "output1", "output2", "output3"].get(..self.child_count())?.get(slot).copied()
    }
}
