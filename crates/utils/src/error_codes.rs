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

/// Trait for defining error code mappings
///
/// Each module should implement this trait to define their specific error codes
pub trait ErrorCodeMapping {
    /// Get the error type for this module
    fn error_type() -> u16;

    /// Get all error codes defined by this module
    fn error_codes() -> Vec<(u16, &'static str)>;

    /// Get the description for a specific error code
    fn error_description(code: u16) -> Option<&'static str>;
}

/// Helper macro to define error codes for a module
///
/// # Example
///
/// ```rust
/// use rustfs_utils::error_codes::{define_error_codes, error_types};
///
/// define_error_codes! {
///     error_type: error_types::FILEMETA,
///     codes: {
///         FILE_NOT_FOUND = 0x0001,
///         FILE_VERSION_NOT_FOUND = 0x0002,
///         VOLUME_NOT_FOUND = 0x0003,
///         FILE_CORRUPT = 0x0004,
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_error_codes {
    (
        error_type: $error_type:expr,
        codes: {
            $(
                $name:ident = $code:expr
            ),* $(,)?
        }
    ) => {
        pub mod error_codes {
            $(
                pub const $name: u16 = $code;
            )*
        }

        impl $crate::error_codes::ErrorCodeMapping for super::Error {
            fn error_type() -> u16 {
                $error_type
            }

            fn error_codes() -> Vec<(u16, &'static str)> {
                vec![
                    $(
                        ($code, stringify!($name)),
                    )*
                ]
            }

            fn error_description(code: u16) -> Option<&'static str> {
                match code {
                    $(
                        $code => Some(stringify!($name)),
                    )*
                    _ => None,
                }
            }
        }
    };
}

/// Helper macro to automatically generate error codes based on enum variant order
///
/// This macro eliminates the need to manually define error codes by automatically
/// assigning sequential codes starting from 0x0001 based on the enum definition order.
///
/// # Example
///
/// ```rust
/// use rustfs_utils::error_codes::{auto_error_codes, error_types};
///
/// #[derive(Debug)]
/// enum MyError {
///     NotFound,
///     InvalidInput,
///     Timeout,
/// }
///
/// auto_error_codes! {
///     error_type: error_types::SYSTEM,
///     enum_name: MyError,
///     variants: [
///         NotFound,
///         InvalidInput,
///         Timeout,
///     ]
/// }
/// ```
#[macro_export]
macro_rules! auto_error_codes {
    (
        error_type: $error_type:expr,
        enum_name: $enum_name:ident,
        variants: [
            $($variant:ident),* $(,)?
        ]
    ) => {
        impl $crate::error_codes::ToErrorCode for $enum_name {
            fn to_error_code(&self) -> $crate::error_codes::ErrorCode {
                let specific_code = match self {
                    $(
                        $enum_name::$variant $(..)? => {
                            $crate::auto_error_codes!(@count_position $variant; $($variant),*)
                        }
                    )*
                };

                $crate::error_codes::ErrorCode::new($error_type, specific_code)
            }
        }

        impl $crate::error_codes::FromErrorCode<$enum_name> for $enum_name {
            fn from_error_code(code: $crate::error_codes::ErrorCode) -> Option<$enum_name> {
                if code.error_type() != $error_type {
                    return None;
                }

                match code.specific_code() {
                    $(
                        $crate::auto_error_codes!(@count_position $variant; $($variant),*) => {
                            $crate::auto_error_codes!(@create_simple_variant $enum_name::$variant)
                        }
                    )*
                    _ => None,
                }
            }
        }
    };

    // Count the position of a variant in the list (1-based)
    (@count_position $target:ident; $($variant:ident),*) => {
        $crate::auto_error_codes!(@count_position_impl $target, 1; $($variant),*)
    };

    (@count_position_impl $target:ident, $count:expr; $first:ident $(, $rest:ident)*) => {
        if stringify!($target) == stringify!($first) {
            $count
        } else {
            $crate::auto_error_codes!(@count_position_impl $target, $count + 1; $($rest),*)
        }
    };

    (@count_position_impl $target:ident, $count:expr;) => {
        $count // Fallback, should not happen in correct usage
    };

    // Create simple variant (only for variants without fields)
    (@create_simple_variant $enum_name:ident::$variant:ident) => {
        Some($enum_name::$variant)
    };
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
        let display = format!("{}", code);
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
}
