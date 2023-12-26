use super::Program;

/// Package containing all programs compiled from a project,
/// 
pub struct Package {
    /// Programs,
    /// 
    pub(crate) programs: Vec<Program>,
}
