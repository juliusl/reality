use std::fmt::Debug;

use runir::prelude::{NodeRepr, HostRepr};

use super::Program;

/// Package containing all programs compiled from a project,
///
pub struct Package {
    /// Programs,
    ///
    pub(crate) programs: Vec<Program>,
}

impl Package {
    /// Searches for a program w/ name,
    ///
    pub fn search(&self, name: impl AsRef<str>) -> Vec<ProgramMatch> {
        let mut matches = vec![];
        for p in self.programs.iter() {
            if p.context()
                .ok()
                .and_then(|a| a.attribute.host())
                .and_then(|a| a.try_address())
                .filter(|p| p.ends_with(name.as_ref()))
                .is_some()
            {
                if let Some(host) = p
                    .context()
                    .ok()
                    .and_then(|a| a.attribute.host())
                {
                    matches.push(ProgramMatch {
                        host,
                        program: p.clone(),
                    });
                }
            }

            matches.extend(p.search(name.as_ref()));
        }
        matches
    }
}

pub struct ProgramMatch {
    pub(crate) host: HostRepr,
    pub program: Program,
}

impl Debug for ProgramMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramMatch")
            .field("node", &self.host)
            .finish()
    }
}
