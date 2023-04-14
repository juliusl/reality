use reality::v2::prelude::*;

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

struct ATest {
    param: usize,
}

#[async_trait]
impl TestA for ATest {
    fn testa(&self) -> reality::Result<()> {
        println!("hello test a {}", self.param);
        Ok(())
    }

    async fn testb(&self) -> reality::Result<()>  {
        println!("hello test b {}", self.param + 4096);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let testa = thunk_testa(ATest { param: 4096 });

    testa.testa()?;
    testa.testb().await
}
