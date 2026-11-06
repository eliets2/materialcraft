use materialcraft_core::Package;

#[derive(Debug)]
pub struct SbsRead {
    pub pkg: Package,
    pub warnings: Vec<String>,
}
