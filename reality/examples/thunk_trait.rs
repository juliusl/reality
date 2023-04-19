use reality::v2::{prelude::*, Visitor};
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

#[derive(Config, Component, Clone)]
#[compile(ThunkTestA)]
#[storage(VecStorage)]
struct ATest {
    param: usize,
}

impl ATest {
    ///
    /// 
    pub fn print_self(&self) -> Result<()> {
        println!("param: {}", self.param);

        Ok(())
    }

    pub async fn print_self_async(&self) -> Result<()> {
        Ok(())
    }
}

impl Visitor for ATest {
}

#[async_trait]
impl TestA for ATest {
    fn testa(&self) -> reality::Result<()> {
        self.print_self()
    }

    async fn testb(&self) -> reality::Result<()>  {
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
