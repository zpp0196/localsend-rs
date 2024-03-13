mod error;
pub mod receive;
pub mod scanner;
pub mod send;
pub mod server;
mod settings;
pub mod util;

pub type Result<T> = std::result::Result<T, error::Error>;

pub use error::*;
pub use settings::*;
