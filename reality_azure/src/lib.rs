
mod block_client;
pub use block_client::AzureBlockClient;

mod block_store;
pub use block_store::AzureBlockStore;

pub mod store;
pub use store::Store;