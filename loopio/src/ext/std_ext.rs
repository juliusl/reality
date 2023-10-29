use std::{path::PathBuf, collections::BTreeMap};

use reality::prelude::*;

pub trait StdExt {
    
}

#[derive(Reality, Default)]
#[reality(rename = "utility/loopio.ext.std.io")]
pub struct Stdio {
    #[reality(map_of=ReadFromStr)]
    read_from_str: BTreeMap<String, ReadFromStr>
}

#[derive(Reality, Default)]
#[reality(rename = "utility/loopio.ext.std.io.read_from_string")]
pub struct ReadFromStr {
    /// Path to read string from,
    /// 
    #[reality(derive_fromstr)]
    path: PathBuf
}


impl std::str::FromStr for Stdio {
    type Err = anyhow::Error;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
       Ok(Stdio::default())
    }
}