/// Struct containing metadata information on a block being parsed,
/// 
#[derive(Hash, Clone, Debug)]
pub struct Info<'a> {
    /// Index of the block,
    /// 
    /// This is the position of the block when it was parsed,
    /// 
    pub idx: usize,
    /// Optional, type string found after the block start,
    /// 
    pub ty: Option<&'a str>,
    /// Optional, moniker found after the type string,
    /// 
    pub moniker: Option<&'a str>,
}