//! Contains the validation logic for handlers.

mod traits;
pub use traits::Validator;

mod error;
pub use error::BlockValidationError;

mod block;
pub use block::BlockValidator;
