use crate::{Session, core};

/// A fresh, unsaved package with one empty graph, used at desktop and browser startup
pub fn new_session() -> Session {
    let mut pkg = core::Package::new("Untitled package");
    pkg.graphs.push(core::Graph::new("New_graph"));
    Session::new(pkg)
}

pub fn sample_session() -> Session {
    match crate::atomic_sample() {
        Ok(package) => Session::new(package),
        Err(error) => {
            let mut session = new_session();
            session.log.push(format!("Could not create the atomic sample: {error}"));
            session
        }
    }
}
