//! Contains the block handler.

mod traits;
pub use traits::Handler;

mod block;
pub use block::BlockHandler;

mod error;
pub use error::HandlerEncodeError;
