//! Error types for reading COMTRADE recordings.

use std::path::PathBuf;

/// Result alias used by the [`crate::comtrade`] module.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while locating, reading or parsing COMTRADE files.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file could not be opened or read.
    #[error("{}: {source}", path.display())]
    Io {
        /// File that failed.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// A file required for the recording does not exist.
    #[error("missing file: {0}")]
    MissingFile(String),

    /// The configuration (`.cfg`) content is invalid.
    #[error("cfg line {line}: {message}")]
    Cfg {
        /// 1-based line number in the `.cfg` content.
        line: usize,
        /// Description of the problem.
        message: String,
    },

    /// The sample data (`.dat`) content is invalid.
    #[error("dat sample {sample}: {message}")]
    Dat {
        /// 1-based sample (record) number.
        sample: usize,
        /// Description of the problem.
        message: String,
    },

    /// The content is valid but uses a feature that is not supported.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl Error {
    /// Creates an [`Error::Io`] for `path`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Returns `true` when the error means a file is missing or unreadable
    /// (as opposed to invalid content).
    pub fn is_file_error(&self) -> bool {
        matches!(self, Self::Io { .. } | Self::MissingFile(_))
    }
}
