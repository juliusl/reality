use reality::v2::prelude::*;
use specs::VecStorage;

/// Test A trait
///
#[thunk]
#[async_trait]
pub trait TestA {
    /// Tests test a,
    ///
    fn testa(&self) -> reality::Result<()>;

    async fn testb(&self) -> reality::Result<()>;
}

#[derive(Runmd, Component, Clone)]
#[compile(TestA)]
#[storage(VecStorage)]
pub struct ATest {
    param: usize,
}

impl ATest {
    pub fn print_self(&self) -> Result<()> {
        println!("param: {}", self.param);
        Ok(())
    }

    pub async fn print_self_async(&self) -> Result<()> {
        println!("from_async -- param: {}", self.param);
        Ok(())
    }
}

#[async_trait]
impl TestA for ATest {
    fn testa(&self) -> reality::Result<()> {
        self.print_self()
    }

    async fn testb(&self) -> reality::Result<()> {
        self.print_self_async().await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let testa = thunk_testa(ATest { param: 4096 });
    // thunk_testa();
    testa.testa()?;
    testa.testb().await
}
