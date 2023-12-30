use tokio::task_local;

task_local! {
    pub static ENTROPY: u64;
}
