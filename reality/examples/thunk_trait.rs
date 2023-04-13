use reality::v2::prelude::*;

thunk! {
    /// Test A trait
    /// 
    #[async_trait]
    pub trait TestA {
        /// Tests test a,
        /// 
        fn testa(&self, test: usize) -> String;
   }
}

fn main() {
    let testa = thunk_testa(ATest{});
    println!("{}", testa.testa(2039));
}

struct ATest;

impl TestA for ATest {
    fn testa(&self, test: usize) -> String {
        format!("hello test a, {test}")
    }
}