//! Error types for the scripting runtime.

use thiserror::Error;

/// Errors that can occur during script execution.
#[derive(Debug, Error)]
pub enum Error {
    /// The Lua VM itself raised an error.
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    /// Script execution failed with an application-level message.
    #[error("script execution failed: {0}")]
    Execution(String),

    /// A script passed an invalid argument to a host API.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
