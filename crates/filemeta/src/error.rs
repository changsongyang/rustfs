// Copyright 2024 RustFS Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use rustfs_utils::error_codes::{AutoErrorCode, ErrorCode, FromErrorCode, ToErrorCode, error_types};

pub type Result<T> = core::result::Result<T, Error>;

// TODO: replace by DiskError
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("File not found")]
    FileNotFound,
    #[error("File version not found")]
    FileVersionNotFound,

    #[error("Volume not found")]
    VolumeNotFound,

    #[error("File corrupt")]
    FileCorrupt,

    #[error("Done for now")]
    DoneForNow,

    #[error("Method not allowed")]
    MethodNotAllowed,

    #[error("Unexpected error")]
    Unexpected,

    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

// Implement AutoErrorCode trait directly
impl AutoErrorCode for Error {
    fn error_type() -> u16 {
        error_types::FILEMETA
    }

    fn variant_index(&self) -> u16 {
        match self {
            Error::FileNotFound => 1,
            Error::FileVersionNotFound => 2,
            Error::VolumeNotFound => 3,
            Error::FileCorrupt => 4,
            Error::DoneForNow => 5,
            Error::MethodNotAllowed => 6,
            Error::Unexpected => 7,
            Error::Io(_) => 8,
        }
    }

    fn from_variant_index(index: u16) -> Option<Self> {
        match index {
            1 => Some(Error::FileNotFound),
            2 => Some(Error::FileVersionNotFound),
            3 => Some(Error::VolumeNotFound),
            4 => Some(Error::FileCorrupt),
            5 => Some(Error::DoneForNow),
            6 => Some(Error::MethodNotAllowed),
            7 => Some(Error::Unexpected),
            8 => Some(Error::Io(std::io::Error::other("I/O error"))),
            _ => None,
        }
    }
}

impl Error {
    pub fn other<E>(error: E) -> Error
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        std::io::Error::other(error).into()
    }

    /// Get the error code using the new u32 format
    pub fn code(&self) -> u32 {
        self.to_error_code().as_u32()
    }

    /// Create an error from a u32 code
    pub fn from_code(code: u32) -> Option<Self> {
        Self::from_error_code(ErrorCode::from_u32(code))
    }

    /// Extract error type from code (high 16 bits)
    pub fn error_type_from_code(code: u32) -> u16 {
        ErrorCode::from_u32(code).error_type()
    }

    /// Extract specific error code (low 16 bits)
    pub fn specific_code_from_code(code: u32) -> u16 {
        ErrorCode::from_u32(code).specific_code()
    }
}

// Note: ToErrorCode and FromErrorCode are automatically implemented via the AutoErrorCode trait

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::FileCorrupt, Error::FileCorrupt) => true,
            (Error::DoneForNow, Error::DoneForNow) => true,
            (Error::MethodNotAllowed, Error::MethodNotAllowed) => true,
            (Error::FileNotFound, Error::FileNotFound) => true,
            (Error::FileVersionNotFound, Error::FileVersionNotFound) => true,
            (Error::VolumeNotFound, Error::VolumeNotFound) => true,
            (Error::Io(e1), Error::Io(e2)) => e1.kind() == e2.kind() && e1.to_string() == e2.to_string(),
            (Error::Unexpected, Error::Unexpected) => true,
            (a, b) => a.to_string() == b.to_string(),
        }
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::FileNotFound => Error::FileNotFound,
            Error::FileVersionNotFound => Error::FileVersionNotFound,
            Error::FileCorrupt => Error::FileCorrupt,
            Error::DoneForNow => Error::DoneForNow,
            Error::MethodNotAllowed => Error::MethodNotAllowed,
            Error::VolumeNotFound => Error::VolumeNotFound,
            Error::Io(e) => Error::Io(std::io::Error::new(e.kind(), e.to_string())),
            Error::Unexpected => Error::Unexpected,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::UnexpectedEof => Error::Unexpected,
            _ => Error::Io(e),
        }
    }
}

impl From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Unexpected => std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Unexpected EOF"),
            Error::Io(e) => e,
            _ => std::io::Error::other(e.to_string()),
        }
    }
}

// Generic error conversion macro for external error types
macro_rules! impl_from_error {
    ($error_type:ty) => {
        impl From<$error_type> for Error {
            fn from(e: $error_type) -> Self {
                Error::other(e.to_string())
            }
        }
    };
    ($error_type:ty, debug) => {
        impl From<$error_type> for Error {
            fn from(e: $error_type) -> Self {
                Error::other(format!("{e:?}"))
            }
        }
    };
}

// Apply the macro to all external error types
impl_from_error!(rmp_serde::decode::Error);
impl_from_error!(rmp_serde::encode::Error);
impl_from_error!(std::string::FromUtf8Error);
impl_from_error!(rmp::decode::ValueReadError);
impl_from_error!(rmp::decode::DecodeStringError<'_>);
impl_from_error!(rmp::encode::ValueWriteError);
impl_from_error!(rmp::decode::NumValueReadError);
impl_from_error!(time::error::ComponentRange);
impl_from_error!(uuid::Error);
impl_from_error!(rmp::decode::MarkerReadError, debug);

pub fn is_io_eof(e: &Error) -> bool {
    match e {
        Error::Io(e) => e.kind() == std::io::ErrorKind::UnexpectedEof,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_filemeta_error_from_io_error() {
        let io_error = IoError::new(ErrorKind::PermissionDenied, "permission denied");
        let filemeta_error: Error = io_error.into();

        match filemeta_error {
            Error::Io(inner_io) => {
                assert_eq!(inner_io.kind(), ErrorKind::PermissionDenied);
                assert!(inner_io.to_string().contains("permission denied"));
            }
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_filemeta_error_other_function() {
        let custom_error = "Custom filemeta error";
        let filemeta_error = Error::other(custom_error);

        match filemeta_error {
            Error::Io(io_error) => {
                assert!(io_error.to_string().contains(custom_error));
                assert_eq!(io_error.kind(), ErrorKind::Other);
            }
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_filemeta_error_clone() {
        let test_cases = vec![
            Error::FileNotFound,
            Error::FileVersionNotFound,
            Error::VolumeNotFound,
            Error::FileCorrupt,
            Error::DoneForNow,
            Error::MethodNotAllowed,
            Error::Unexpected,
            Error::Io(IoError::new(ErrorKind::NotFound, "test")),
        ];

        for original_error in test_cases {
            let cloned_error = original_error.clone();
            assert_eq!(original_error, cloned_error);
        }
    }

    #[test]
    fn test_filemeta_error_partial_eq() {
        // Test equality for simple variants
        assert_eq!(Error::FileNotFound, Error::FileNotFound);
        assert_ne!(Error::FileNotFound, Error::FileVersionNotFound);

        // Test equality for Io variants
        let io1 = Error::Io(IoError::new(ErrorKind::NotFound, "test"));
        let io2 = Error::Io(IoError::new(ErrorKind::NotFound, "test"));
        let io3 = Error::Io(IoError::new(ErrorKind::PermissionDenied, "test"));
        assert_eq!(io1, io2);
        assert_ne!(io1, io3);
    }

    #[test]
    fn test_filemeta_error_display() {
        let test_cases = vec![
            (Error::FileNotFound, "File not found"),
            (Error::FileVersionNotFound, "File version not found"),
            (Error::VolumeNotFound, "Volume not found"),
            (Error::FileCorrupt, "File corrupt"),
            (Error::DoneForNow, "Done for now"),
            (Error::MethodNotAllowed, "Method not allowed"),
            (Error::Unexpected, "Unexpected error"),
        ];

        for (error, expected_message) in test_cases {
            assert_eq!(error.to_string(), expected_message);
        }
    }

    #[test]
    fn test_is_io_eof_function() {
        // Test is_io_eof helper function
        let eof_error = Error::Io(IoError::new(ErrorKind::UnexpectedEof, "eof"));
        assert!(is_io_eof(&eof_error));

        let not_eof_error = Error::Io(IoError::new(ErrorKind::NotFound, "not found"));
        assert!(!is_io_eof(&not_eof_error));

        let non_io_error = Error::FileNotFound;
        assert!(!is_io_eof(&non_io_error));
    }

    #[test]
    fn test_filemeta_error_to_io_error_conversion() {
        // Test conversion from FileMeta Error to io::Error through other function
        let original_io_error = IoError::new(ErrorKind::InvalidData, "test data");
        let filemeta_error = Error::other(original_io_error);

        match filemeta_error {
            Error::Io(io_err) => {
                assert_eq!(io_err.kind(), ErrorKind::Other);
                assert!(io_err.to_string().contains("test data"));
            }
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_filemeta_error_roundtrip_conversion() {
        // Test roundtrip conversion: io::Error -> FileMeta Error -> io::Error
        let original_io_error = IoError::new(ErrorKind::PermissionDenied, "permission test");

        // Convert to FileMeta Error
        let filemeta_error: Error = original_io_error.into();

        // Extract the io::Error back
        match filemeta_error {
            Error::Io(extracted_io_error) => {
                assert_eq!(extracted_io_error.kind(), ErrorKind::PermissionDenied);
                assert!(extracted_io_error.to_string().contains("permission test"));
            }
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_filemeta_error_io_error_kinds_preservation() {
        let io_error_kinds = vec![
            ErrorKind::NotFound,
            ErrorKind::PermissionDenied,
            ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::NotConnected,
            ErrorKind::AddrInUse,
            ErrorKind::AddrNotAvailable,
            ErrorKind::BrokenPipe,
            ErrorKind::AlreadyExists,
            ErrorKind::WouldBlock,
            ErrorKind::InvalidInput,
            ErrorKind::InvalidData,
            ErrorKind::TimedOut,
            ErrorKind::WriteZero,
            ErrorKind::Interrupted,
            ErrorKind::UnexpectedEof,
            ErrorKind::Other,
        ];

        for kind in io_error_kinds {
            let io_error = IoError::new(kind, format!("test error for {kind:?}"));
            let filemeta_error: Error = io_error.into();

            match filemeta_error {
                Error::Unexpected => {
                    assert_eq!(kind, ErrorKind::UnexpectedEof);
                }
                Error::Io(extracted_io_error) => {
                    assert_eq!(extracted_io_error.kind(), kind);
                    assert!(extracted_io_error.to_string().contains("test error"));
                }
                _ => panic!("Expected Io variant for kind {kind:?}"),
            }
        }
    }

    #[test]
    fn test_filemeta_error_downcast_chain() {
        // Test error downcast chain functionality
        let original_io_error = IoError::new(ErrorKind::InvalidData, "original error");
        let filemeta_error = Error::other(original_io_error);

        // The error should be wrapped as an Io variant
        if let Error::Io(io_err) = filemeta_error {
            // The wrapped error should be Other kind (from std::io::Error::other)
            assert_eq!(io_err.kind(), ErrorKind::Other);
            // But the message should still contain the original error information
            assert!(io_err.to_string().contains("original error"));
        } else {
            panic!("Expected Io variant");
        }
    }

    #[test]
    fn test_filemeta_error_maintains_error_information() {
        let test_cases = vec![
            (ErrorKind::NotFound, "file not found"),
            (ErrorKind::PermissionDenied, "access denied"),
            (ErrorKind::InvalidData, "corrupt data"),
            (ErrorKind::TimedOut, "operation timed out"),
        ];

        for (kind, message) in test_cases {
            let io_error = IoError::new(kind, message);
            let error_message = io_error.to_string();
            let filemeta_error: Error = io_error.into();

            match filemeta_error {
                Error::Io(extracted_io_error) => {
                    assert_eq!(extracted_io_error.kind(), kind);
                    assert_eq!(extracted_io_error.to_string(), error_message);
                }
                _ => panic!("Expected Io variant"),
            }
        }
    }

    #[test]
    fn test_filemeta_error_equality_with_io_errors() {
        // Test equality comparison for Io variants
        let io_error1 = IoError::new(ErrorKind::NotFound, "test message");
        let io_error2 = IoError::new(ErrorKind::NotFound, "test message");
        let io_error3 = IoError::new(ErrorKind::PermissionDenied, "test message");
        let io_error4 = IoError::new(ErrorKind::NotFound, "different message");

        let filemeta_error1 = Error::Io(io_error1);
        let filemeta_error2 = Error::Io(io_error2);
        let filemeta_error3 = Error::Io(io_error3);
        let filemeta_error4 = Error::Io(io_error4);

        // Same kind and message should be equal
        assert_eq!(filemeta_error1, filemeta_error2);

        // Different kinds should not be equal
        assert_ne!(filemeta_error1, filemeta_error3);

        // Different messages should not be equal
        assert_ne!(filemeta_error1, filemeta_error4);
    }

    #[test]
    fn test_filemeta_error_clone_io_variants() {
        let io_error = IoError::new(ErrorKind::ConnectionReset, "connection lost");
        let original_error = Error::Io(io_error);
        let cloned_error = original_error.clone();

        // Cloned error should be equal to original
        assert_eq!(original_error, cloned_error);

        // Both should maintain the same properties
        match (original_error, cloned_error) {
            (Error::Io(orig_io), Error::Io(cloned_io)) => {
                assert_eq!(orig_io.kind(), cloned_io.kind());
                assert_eq!(orig_io.to_string(), cloned_io.to_string());
            }
            _ => panic!("Both should be Io variants"),
        }
    }

    #[test]
    fn test_error_code_auto_generation() {
        // Test that error codes are generated automatically based on enum variant order
        let test_cases = vec![
            (Error::FileNotFound, 1),
            (Error::FileVersionNotFound, 2),
            (Error::VolumeNotFound, 3),
            (Error::FileCorrupt, 4),
            (Error::DoneForNow, 5),
            (Error::MethodNotAllowed, 6),
            (Error::Unexpected, 7),
            (Error::Io(std::io::Error::other("test")), 8),
        ];

        for (error, expected_code) in test_cases {
            let error_code = error.to_error_code();
            assert_eq!(error_code.error_type(), error_types::FILEMETA);
            assert_eq!(error_code.specific_code(), expected_code);

            // Test round-trip conversion
            let reconstructed = Error::from_error_code(error_code);
            assert!(reconstructed.is_some());
        }
    }

    #[test]
    fn test_error_code_uniqueness() {
        // Test that all error variants have unique codes
        let errors = vec![
            Error::FileNotFound,
            Error::FileVersionNotFound,
            Error::VolumeNotFound,
            Error::FileCorrupt,
            Error::DoneForNow,
            Error::MethodNotAllowed,
            Error::Unexpected,
            Error::Io(std::io::Error::other("test")),
        ];

        let mut codes = std::collections::HashSet::new();
        for error in errors {
            let code = error.to_error_code().specific_code();
            assert!(codes.insert(code), "Duplicate error code found: {code}");
        }
    }
}
