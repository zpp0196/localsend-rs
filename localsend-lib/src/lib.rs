mod error;
pub mod scanner;
pub mod send;
pub mod server;
pub mod util;

pub type Result<T> = std::result::Result<T, error::Error>;

pub use error::*;
