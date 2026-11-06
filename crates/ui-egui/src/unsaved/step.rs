use super::Guarded;

/// What to do after an answer
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// Run the action now
    Run(Guarded),
    /// Save first, then run the action if the package is no longer modified
    Save(Guarded),
    /// Do nothing
    Stop,
}
