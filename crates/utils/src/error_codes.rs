//! # RustFS Error Code System
//!
//! This module defines the error code system for the RustFS project.
//! The error codes use a u32 format where:
//! - High 16 bits (31-16): Error type/category
//! - Low 16 bits (15-0): Specific error code within that category
//!
//! ## Design Philosophy
//!
//! - **Centralized Type Definition**: Error types are defined here to ensure uniqueness
//! - **Decentralized Implementation**: Each module implements its own specific error codes
//! - **Extensible Interface**: New error types can be easily added without breaking existing code
//!
//! ## Usage Example for Other Modules
//!
//! Here's how other modules can use the AutoErrorCode trait:
//!
//! ```rust
//! use rustfs_utils::error_codes::{AutoErrorCode, ErrorCode, ToErrorCode, FromErrorCode, error_types};
//!
//! #[derive(Debug, PartialEq)]
//! pub enum StorageError {
//!     BucketNotFound(String),
//!     ObjectNotFound { bucket: String, key: String },
//!     StorageFull,
//!     InvalidRequest,
//! }
//!
//! impl std::fmt::Display for StorageError {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         match self {
//!             StorageError::BucketNotFound(bucket) => write!(f, "Bucket not found: {}", bucket),
//!             StorageError::ObjectNotFound { bucket, key } => write!(f, "Object not found: {}/{}", bucket, key),
//!             StorageError::StorageFull => write!(f, "Storage full"),
//!             StorageError::InvalidRequest => write!(f, "Invalid request"),
//!         }
//!     }
//! }
//!
//! impl std::error::Error for StorageError {}
//!
//! impl AutoErrorCode for StorageError {
//!     fn error_type() -> u16 {
//!         error_types::STORAGE
//!     }
//!
//!     fn variant_index(&self) -> u16 {
//!         match self {
//!             StorageError::BucketNotFound(_) => 1,
//!             StorageError::ObjectNotFound { .. } => 2,
//!             StorageError::StorageFull => 3,
//!             StorageError::InvalidRequest => 4,
//!         }
//!     }
//!
//!     fn from_variant_index(index: u16) -> Option<Self> {
//!         match index {
//!             1 => Some(StorageError::BucketNotFound("unknown".to_string())),
//!             2 => Some(StorageError::ObjectNotFound {
//!                 bucket: "unknown".to_string(),
//!                 key: "unknown".to_string(),
//!             }),
//!             3 => Some(StorageError::StorageFull),
//!             4 => Some(StorageError::InvalidRequest),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! // Now you can use the error codes:
//! let error = StorageError::BucketNotFound("my-bucket".to_string());
//! let code = error.to_error_code();
//! assert_eq!(code.as_u32(), 0x0002_0001); // STORAGE type (0x0002) + variant index (0x0001)
//!
//! // And convert back:
//! let reconstructed = StorageError::from_error_code(code);
//! assert!(reconstructed.is_some());
//! ```

use std::fmt;

/// Error type constants (high 16 bits)
///
/// These constants define the error type categories used across the RustFS system.
/// Each module should use one of these types and implement their own specific error codes.
pub mod error_types {
    /// System-level errors (0x0000xxxx)
    pub const SYSTEM: u16 = 0x0000;

    /// File metadata errors (0x0001xxxx)
    pub const FILEMETA: u16 = 0x0001;

    /// Storage layer errors (0x0002xxxx)
    pub const STORAGE: u16 = 0x0002;

    /// Disk operation errors (0x0003xxxx)
    pub const DISK: u16 = 0x0003;

    /// Identity and access management errors (0x0004xxxx)
    pub const IAM: u16 = 0x0004;

    /// Policy-related errors (0x0005xxxx)
    pub const POLICY: u16 = 0x0005;

    /// Cryptographic errors (0x0006xxxx)
    pub const CRYPTO: u16 = 0x0006;

    /// Notification system errors (0x0007xxxx)
    pub const NOTIFY: u16 = 0x0007;

    /// API layer errors (0x0008xxxx)
    pub const API: u16 = 0x0008;

    /// Network communication errors (0x0009xxxx)
    pub const NETWORK: u16 = 0x0009;

    /// Configuration errors (0x000Axxxx)
    pub const CONFIG: u16 = 0x000A;

    /// Authentication errors (0x000Bxxxx)
    pub const AUTH: u16 = 0x000B;

    /// Bucket metadata errors (0x000Cxxxx)
    pub const BUCKET: u16 = 0x000C;

    /// Object operation errors (0x000Dxxxx)
    pub const OBJECT: u16 = 0x000D;

    /// Query/SQL errors (0x000Exxxx)
    pub const QUERY: u16 = 0x000E;

    /// Admin operation errors (0x000Fxxxx)
    pub const ADMIN: u16 = 0x000F;

    // Reserved range: 0x0010-0xFFFF for future use
}

/// Error code structure that combines error type and specific code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorCode {
    code: u32,
}

impl ErrorCode {
    /// Create a new error code from error type and specific code
    pub fn new(error_type: u16, specific_code: u16) -> Self {
        Self {
            code: ((error_type as u32) << 16) | (specific_code as u32),
        }
    }

    /// Create from a u32 value
    pub fn from_u32(code: u32) -> Self {
        Self { code }
    }

    /// Get the full error code as u32
    pub fn as_u32(&self) -> u32 {
        self.code
    }

    /// Get the error type (high 16 bits)
    pub fn error_type(&self) -> u16 {
        (self.code >> 16) as u16
    }

    /// Get the specific error code (low 16 bits)
    pub fn specific_code(&self) -> u16 {
        (self.code & 0x0000_FFFF) as u16
    }

    /// Get the error type name
    pub fn error_type_name(&self) -> &'static str {
        match self.error_type() {
            error_types::SYSTEM => "System",
            error_types::FILEMETA => "FileMeta",
            error_types::STORAGE => "Storage",
            error_types::DISK => "Disk",
            error_types::IAM => "IAM",
            error_types::POLICY => "Policy",
            error_types::CRYPTO => "Crypto",
            error_types::NOTIFY => "Notify",
            error_types::API => "API",
            error_types::NETWORK => "Network",
            error_types::CONFIG => "Config",
            error_types::AUTH => "Auth",
            error_types::BUCKET => "Bucket",
            error_types::OBJECT => "Object",
            error_types::QUERY => "Query",
            error_types::ADMIN => "Admin",
            _ => "Unknown",
        }
    }

    /// Check if this is a system error
    pub fn is_system_error(&self) -> bool {
        self.error_type() == error_types::SYSTEM
    }

    /// Check if this is a storage-related error
    pub fn is_storage_error(&self) -> bool {
        matches!(self.error_type(), error_types::STORAGE | error_types::DISK | error_types::FILEMETA)
    }

    /// Check if this is an authentication/authorization error
    pub fn is_auth_error(&self) -> bool {
        matches!(self.error_type(), error_types::IAM | error_types::POLICY | error_types::AUTH)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{:04X}:{:04X}", self.error_type_name(), self.error_type(), self.specific_code())
    }
}

/// Trait for converting errors to error codes
///
/// Each error type should implement this trait to provide error code conversion
pub trait ToErrorCode {
    /// Convert this error to an error code
    fn to_error_code(&self) -> ErrorCode;

    /// Get the error code as u32 (convenience method)
    fn error_code_u32(&self) -> u32 {
        self.to_error_code().as_u32()
    }
}

/// Trait for converting error codes to errors
///
/// Each error type should implement this trait to support error code to error conversion
pub trait FromErrorCode<T> {
    /// Try to convert an error code to this error type
    /// Returns None if the error code doesn't belong to this error type
    fn from_error_code(code: ErrorCode) -> Option<T>;

    /// Try to convert a u32 error code to this error type (convenience method)
    fn from_error_code_u32(code: u32) -> Option<T> {
        Self::from_error_code(ErrorCode::from_u32(code))
    }
}

/// Trait for automatic error code generation based on enum variant order
///
/// This trait provides automatic error code generation and parsing for enums,
/// eliminating the need for manual error code definitions.
pub trait AutoErrorCode: Sized {
    /// Get the error type for this enum
    fn error_type() -> u16;

    /// Get the variant index based on enum definition order (1-based)
    fn variant_index(&self) -> u16;

    /// Create an error from a variant index
    fn from_variant_index(index: u16) -> Option<Self>;
}

/// Blanket implementation of ToErrorCode for types that implement AutoErrorCode
impl<T: AutoErrorCode> ToErrorCode for T {
    fn to_error_code(&self) -> ErrorCode {
        let specific_code = self.variant_index();
        ErrorCode::new(T::error_type(), specific_code)
    }
}

/// Blanket implementation of FromErrorCode for types that implement AutoErrorCode
impl<T: AutoErrorCode> FromErrorCode<T> for T {
    fn from_error_code(code: ErrorCode) -> Option<T> {
        if code.error_type() != T::error_type() {
            return None;
        }
        T::from_variant_index(code.specific_code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_creation() {
        let code = ErrorCode::new(error_types::FILEMETA, 0x0001);
        assert_eq!(code.error_type(), error_types::FILEMETA);
        assert_eq!(code.specific_code(), 0x0001);
        assert_eq!(code.as_u32(), 0x0001_0001);
    }

    #[test]
    fn test_error_code_from_u32() {
        let code = ErrorCode::from_u32(0x0002_0005);
        assert_eq!(code.error_type(), error_types::STORAGE);
        assert_eq!(code.specific_code(), 0x0005);
    }

    #[test]
    fn test_error_type_name() {
        let code = ErrorCode::new(error_types::CRYPTO, 0x0001);
        assert_eq!(code.error_type_name(), "Crypto");
    }

    #[test]
    fn test_error_classifications() {
        let system_error = ErrorCode::new(error_types::SYSTEM, 0x0001);
        assert!(system_error.is_system_error());

        let storage_error = ErrorCode::new(error_types::STORAGE, 0x0001);
        assert!(storage_error.is_storage_error());

        let auth_error = ErrorCode::new(error_types::IAM, 0x0001);
        assert!(auth_error.is_auth_error());
    }

    #[test]
    fn test_error_code_display() {
        let code = ErrorCode::new(error_types::FILEMETA, 0x0001);
        let display = format!("{code}");
        assert_eq!(display, "FileMeta:0001:0001");
    }

    #[test]
    fn test_all_error_type_constants() {
        // Test that all error type constants are unique and in expected range
        let types = [
            error_types::SYSTEM,
            error_types::FILEMETA,
            error_types::STORAGE,
            error_types::DISK,
            error_types::IAM,
            error_types::POLICY,
            error_types::CRYPTO,
            error_types::NOTIFY,
            error_types::API,
            error_types::NETWORK,
            error_types::CONFIG,
            error_types::AUTH,
            error_types::BUCKET,
            error_types::OBJECT,
            error_types::QUERY,
            error_types::ADMIN,
        ];

        for (i, &type_code) in types.iter().enumerate() {
            assert_eq!(type_code, i as u16);
        }
    }

    #[test]
    fn test_error_code_ranges() {
        // Test that error codes don't overlap
        let filemeta_code = ErrorCode::new(error_types::FILEMETA, 0x0001);
        let storage_code = ErrorCode::new(error_types::STORAGE, 0x0001);

        assert_ne!(filemeta_code.as_u32(), storage_code.as_u32());
        assert_eq!(filemeta_code.as_u32(), 0x0001_0001);
        assert_eq!(storage_code.as_u32(), 0x0002_0001);
    }

    #[test]
    fn test_auto_error_code_trait() {
        // Create a simple test enum to verify the AutoErrorCode trait
        #[derive(Debug, PartialEq)]
        enum TestError {
            First,
            Second,
            Third,
        }

        impl AutoErrorCode for TestError {
            fn error_type() -> u16 {
                error_types::SYSTEM
            }

            fn variant_index(&self) -> u16 {
                match self {
                    TestError::First => 1,
                    TestError::Second => 2,
                    TestError::Third => 3,
                }
            }

            fn from_variant_index(index: u16) -> Option<Self> {
                match index {
                    1 => Some(TestError::First),
                    2 => Some(TestError::Second),
                    3 => Some(TestError::Third),
                    _ => None,
                }
            }
        }

        // Test ToErrorCode implementation
        let error = TestError::Second;
        let code = error.to_error_code();
        assert_eq!(code.error_type(), error_types::SYSTEM);
        assert_eq!(code.specific_code(), 2);

        // Test FromErrorCode implementation
        let reconstructed = TestError::from_error_code(code);
        assert_eq!(reconstructed, Some(TestError::Second));

        // Test invalid error type
        let invalid_code = ErrorCode::new(error_types::FILEMETA, 2);
        let result = TestError::from_error_code(invalid_code);
        assert_eq!(result, None);

        // Test invalid specific code
        let invalid_specific = ErrorCode::new(error_types::SYSTEM, 99);
        let result = TestError::from_error_code(invalid_specific);
        assert_eq!(result, None);
    }

    #[test]
    fn test_auto_error_code_blanket_implementations() {
        // Test that the blanket implementations work correctly
        #[derive(Debug, PartialEq)]
        enum AnotherTestError {
            NotFound,
            InvalidInput,
        }

        impl AutoErrorCode for AnotherTestError {
            fn error_type() -> u16 {
                error_types::STORAGE
            }

            fn variant_index(&self) -> u16 {
                match self {
                    AnotherTestError::NotFound => 1,
                    AnotherTestError::InvalidInput => 2,
                }
            }

            fn from_variant_index(index: u16) -> Option<Self> {
                match index {
                    1 => Some(AnotherTestError::NotFound),
                    2 => Some(AnotherTestError::InvalidInput),
                    _ => None,
                }
            }
        }

        let error = AnotherTestError::InvalidInput;

        // Test that ToErrorCode works via blanket implementation
        let code = error.to_error_code();
        assert_eq!(code.error_type(), error_types::STORAGE);
        assert_eq!(code.specific_code(), 2);
        assert_eq!(code.as_u32(), 0x0002_0002);

        // Test that FromErrorCode works via blanket implementation
        let reconstructed = AnotherTestError::from_error_code(code);
        assert_eq!(reconstructed, Some(AnotherTestError::InvalidInput));

        // Test convenience methods
        assert_eq!(error.error_code_u32(), 0x0002_0002);
        assert_eq!(AnotherTestError::from_error_code_u32(0x0002_0001), Some(AnotherTestError::NotFound));
    }
}
