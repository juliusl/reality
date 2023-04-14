use reality::v2::prelude::*;

/// Test A trait
///
#[thunk]
pub trait TestA {
    /// Tests test a,
    ///
    fn testa(&self) -> reality::Result<()>;
}

fn main() {
    let testa = thunk_testa(ATest {param: 4096});
    println!("{:?}", testa.testa());
}

struct ATest {
    param: usize
}

impl TestA for ATest {
    fn testa(&self) -> reality::Result<()> {
        println!("hello test a {}", self.param);
        Ok(())
    }
}
