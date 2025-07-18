# RustFS 错误代码系统设计文档

## 概述

RustFS 项目采用分层的错误代码系统，使用 u32 格式存储错误信息：
- **高16位 (31-16)**：错误类型/分类（在 `rustfs-utils` 中统一定义）
- **低16位 (15-0)**：具体错误代码（由各业务模块自己实现）

## 设计哲学

- **集中化类型定义**：错误类型在 `rustfs-utils` 中定义，确保唯一性
- **分布式实现**：各模块实现自己的具体错误代码
- **可扩展接口**：新的错误类型可以轻松添加，而不会破坏现有代码

## 错误代码格式

```
31                    16 15                     0
+----------------------+----------------------+
|     Error Type       |   Specific Code      |
+----------------------+----------------------+
```

### 示例
- `0x00010001` = FileMeta 类型的 FILE_NOT_FOUND 错误
- `0x00020005` = Storage 类型的 FILE_NOT_FOUND 错误
- `0x00060401` = Crypto 类型的 ENCRYPT_FAILED 错误

## 错误类型分类（在 rustfs-utils 中定义）

| 类型 | 代码 | 描述 |
|------|------|------|
| System | 0x0000 | 系统级错误 |
| FileMeta | 0x0001 | 文件元数据错误 |
| Storage | 0x0002 | 存储层错误 |
| Disk | 0x0003 | 磁盘操作错误 |
| IAM | 0x0004 | 身份认证管理 |
| Policy | 0x0005 | 策略相关错误 |
| Crypto | 0x0006 | 加密操作错误 |
| Notify | 0x0007 | 通知系统错误 |
| API | 0x0008 | API 层错误 |
| Network | 0x0009 | 网络通信错误 |
| Config | 0x000A | 配置错误 |
| Auth | 0x000B | 认证错误 |
| Bucket | 0x000C | 存储桶元数据错误 |
| Object | 0x000D | 对象操作错误 |
| Query | 0x000E | 查询/SQL 错误 |
| Admin | 0x000F | 管理操作错误 |
| Reserved | 0x0010-0xFFFF | 保留给未来使用 |

## 通用错误代码范围（在 rustfs-utils 中定义）

为了保持一致性，`rustfs-utils` 提供了通用的错误代码范围：

### 基础错误代码 (0x0001-0x00FF)
```rust
pub mod basic {
    pub const NOT_FOUND: u16 = 0x0001;
    pub const ALREADY_EXISTS: u16 = 0x0002;
    pub const ACCESS_DENIED: u16 = 0x0003;
    pub const CORRUPTED: u16 = 0x0004;
    pub const METHOD_NOT_ALLOWED: u16 = 0x0005;
    pub const INVALID_ARGUMENT: u16 = 0x0006;
    pub const TIMEOUT: u16 = 0x0007;
    pub const CANCELLED: u16 = 0x0008;
    pub const UNAVAILABLE: u16 = 0x0009;
    pub const INTERNAL_ERROR: u16 = 0x000A;
    pub const NOT_IMPLEMENTED: u16 = 0x000B;
    pub const UNEXPECTED: u16 = 0x000C;
}
```

### I/O 相关错误 (0x0100-0x01FF)
```rust
pub mod io {
    pub const IO_ERROR: u16 = 0x0100;
    pub const READ_ERROR: u16 = 0x0101;
    pub const WRITE_ERROR: u16 = 0x0102;
    pub const SEEK_ERROR: u16 = 0x0103;
    pub const FLUSH_ERROR: u16 = 0x0104;
    pub const CLOSE_ERROR: u16 = 0x0105;
}
```

### 网络相关错误 (0x0200-0x02FF)
```rust
pub mod network {
    pub const NETWORK_ERROR: u16 = 0x0200;
    pub const CONNECTION_FAILED: u16 = 0x0201;
    pub const CONNECTION_LOST: u16 = 0x0202;
    pub const TIMEOUT: u16 = 0x0203;
    pub const DNS_ERROR: u16 = 0x0204;
    pub const TLS_ERROR: u16 = 0x0205;
}
```

### 序列化/反序列化错误 (0x0300-0x03FF)
```rust
pub mod serde {
    pub const SERIALIZE_ERROR: u16 = 0x0300;
    pub const DESERIALIZE_ERROR: u16 = 0x0301;
    pub const INVALID_FORMAT: u16 = 0x0302;
    pub const ENCODING_ERROR: u16 = 0x0303;
    pub const DECODING_ERROR: u16 = 0x0304;
}
```

### 加密相关错误 (0x0400-0x04FF)
```rust
pub mod crypto {
    pub const CRYPTO_ERROR: u16 = 0x0400;
    pub const ENCRYPT_FAILED: u16 = 0x0401;
    pub const DECRYPT_FAILED: u16 = 0x0402;
    pub const INVALID_KEY: u16 = 0x0403;
    pub const INVALID_SIGNATURE: u16 = 0x0404;
    pub const KEY_GENERATION_FAILED: u16 = 0x0405;
}
```

## 使用方法

### 1. 在业务模块中定义错误代码

```rust
// 在 crates/filemeta/src/error.rs 中
use rustfs_utils::error_codes::{error_types, ErrorCode, ToErrorCode, FromErrorCode, common_ranges};

pub mod error_codes {
    use rustfs_utils::error_codes::common_ranges;
    
    // 使用通用错误代码
    pub const FILE_NOT_FOUND: u16 = common_ranges::basic::NOT_FOUND;
    pub const FILE_CORRUPT: u16 = common_ranges::basic::CORRUPTED;
    pub const IO_ERROR: u16 = common_ranges::io::IO_ERROR;
    
    // 定义模块特定的错误代码
    pub const FILE_VERSION_NOT_FOUND: u16 = 0x0002;
    pub const VOLUME_NOT_FOUND: u16 = 0x0003;
    pub const DONE_FOR_NOW: u16 = 0x0005;
    
    // 序列化相关错误
    pub const RMP_DECODE_VALUE_READ: u16 = 0x0304;
    pub const RMP_ENCODE_VALUE_WRITE: u16 = 0x0305;
    pub const UUID_PARSE: u16 = 0x0309;
}
```

### 2. 实现错误转换 traits

```rust
impl ToErrorCode for Error {
    fn to_error_code(&self) -> ErrorCode {
        let specific_code = match self {
            Error::FileNotFound => error_codes::FILE_NOT_FOUND,
            Error::FileCorrupt => error_codes::FILE_CORRUPT,
            Error::Io(_) => error_codes::IO_ERROR,
            // ... 其他错误映射
        };
        
        ErrorCode::new(error_types::FILEMETA, specific_code)
    }
}

impl FromErrorCode<Error> for Error {
    fn from_error_code(code: ErrorCode) -> Option<Error> {
        if code.error_type() != error_types::FILEMETA {
            return None;
        }
        
        match code.specific_code() {
            error_codes::FILE_NOT_FOUND => Some(Error::FileNotFound),
            error_codes::FILE_CORRUPT => Some(Error::FileCorrupt),
            error_codes::IO_ERROR => Some(Error::Io(std::io::Error::other("I/O error"))),
            // ... 其他错误转换
            _ => None,
        }
    }
}
```

### 3. 基本使用

```rust
use rustfs_utils::error_codes::{ErrorCode, error_types};
use rustfs_filemeta::error::Error as FilemetaError;

// 创建错误代码
let error = FilemetaError::FileNotFound;
let code = error.to_error_code();
println!("Error code: {}", code); // "FileMeta:0001:0001"

// 从错误代码恢复错误
let recovered_error = FilemetaError::from_error_code(code);
match recovered_error {
    Some(FilemetaError::FileNotFound) => println!("Successfully recovered error"),
    _ => println!("Failed to recover error"),
}

// 错误分类检查
if code.is_storage_error() {
    println!("This is a storage-related error");
}
```

### 4. 使用宏简化定义（可选）

```rust
use rustfs_utils::define_error_codes;

define_error_codes! {
    error_type: error_types::FILEMETA,
    codes: {
        FILE_NOT_FOUND = 0x0001,
        FILE_VERSION_NOT_FOUND = 0x0002,
        VOLUME_NOT_FOUND = 0x0003,
        FILE_CORRUPT = 0x0004,
    }
}
```

## 模块实现示例

### FileMeta 模块

```rust
// crates/filemeta/src/error.rs
use rustfs_utils::error_codes::{error_types, ErrorCode, ToErrorCode, FromErrorCode, common_ranges};

pub mod error_codes {
    use rustfs_utils::error_codes::common_ranges;
    
    // 基础错误
    pub const FILE_NOT_FOUND: u16 = common_ranges::basic::NOT_FOUND;
    pub const FILE_CORRUPT: u16 = common_ranges::basic::CORRUPTED;
    pub const METHOD_NOT_ALLOWED: u16 = common_ranges::basic::METHOD_NOT_ALLOWED;
    
    // I/O 错误
    pub const IO_ERROR: u16 = common_ranges::io::IO_ERROR;
    
    // 序列化错误
    pub const RMP_SERDE_DECODE: u16 = common_ranges::serde::DESERIALIZE_ERROR;
    pub const RMP_SERDE_ENCODE: u16 = common_ranges::serde::SERIALIZE_ERROR;
    
    // 模块特定错误
    pub const FILE_VERSION_NOT_FOUND: u16 = 0x0002;
    pub const VOLUME_NOT_FOUND: u16 = 0x0003;
    pub const DONE_FOR_NOW: u16 = 0x0005;
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("File not found")]
    FileNotFound,
    #[error("File corrupt")]
    FileCorrupt,
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    // ... 其他错误
}

impl ToErrorCode for Error {
    fn to_error_code(&self) -> ErrorCode {
        let specific_code = match self {
            Error::FileNotFound => error_codes::FILE_NOT_FOUND,
            Error::FileCorrupt => error_codes::FILE_CORRUPT,
            Error::Io(_) => error_codes::IO_ERROR,
            // ... 其他映射
        };
        ErrorCode::new(error_types::FILEMETA, specific_code)
    }
}
```

### Storage 模块（示例）

```rust
// crates/ecstore/src/error.rs
use rustfs_utils::error_codes::{error_types, ErrorCode, ToErrorCode, FromErrorCode, common_ranges};

pub mod error_codes {
    use rustfs_utils::error_codes::common_ranges;
    
    // 基础错误
    pub const FILE_NOT_FOUND: u16 = common_ranges::basic::NOT_FOUND;
    pub const BUCKET_NOT_FOUND: u16 = 0x001B;  // Storage 特定的错误代码
    pub const STORAGE_FULL: u16 = 0x0020;
    
    // 磁盘相关错误
    pub const FAULTY_DISK: u16 = 0x0001;
    pub const DISK_FULL: u16 = 0x0002;
    pub const CORRUPTED_FORMAT: u16 = 0x000D;
}

#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("File not found")]
    FileNotFound,
    #[error("Bucket not found: {0}")]
    BucketNotFound(String),
    #[error("Storage full")]
    StorageFull,
    // ... 其他错误
}

impl ToErrorCode for StorageError {
    fn to_error_code(&self) -> ErrorCode {
        let specific_code = match self {
            StorageError::FileNotFound => error_codes::FILE_NOT_FOUND,
            StorageError::BucketNotFound(_) => error_codes::BUCKET_NOT_FOUND,
            StorageError::StorageFull => error_codes::STORAGE_FULL,
            // ... 其他映射
        };
        ErrorCode::new(error_types::STORAGE, specific_code)
    }
}
```

## 最佳实践

### 1. 错误代码分配策略

- **优先使用通用错误代码**：对于常见错误（如 NOT_FOUND, ACCESS_DENIED），使用 `common_ranges` 中定义的代码
- **模块特定错误**：为模块特有的错误分配专用代码
- **保持一致性**：同类错误在不同模块中应使用相同的代码（如都使用 `NOT_FOUND`）

### 2. 错误代码范围规划

```rust
// 每个模块的错误代码分配建议
pub mod error_codes {
    // 0x0001-0x00FF: 基础错误（优先使用 common_ranges）
    pub const FILE_NOT_FOUND: u16 = common_ranges::basic::NOT_FOUND;
    pub const ACCESS_DENIED: u16 = common_ranges::basic::ACCESS_DENIED;
    
    // 0x0100-0x01FF: I/O 相关错误
    pub const IO_ERROR: u16 = common_ranges::io::IO_ERROR;
    
    // 0x0200-0x02FF: 网络相关错误
    pub const NETWORK_ERROR: u16 = common_ranges::network::NETWORK_ERROR;
    
    // 0x0300-0x03FF: 序列化相关错误
    pub const SERIALIZE_ERROR: u16 = common_ranges::serde::SERIALIZE_ERROR;
    
    // 0x0400-0x04FF: 加密相关错误
    pub const CRYPTO_ERROR: u16 = common_ranges::crypto::CRYPTO_ERROR;
    
    // 0x1000-0xFFFF: 模块特定错误
    pub const MODULE_SPECIFIC_ERROR: u16 = 0x1000;
}
```

### 3. 错误消息和日志

```rust
use rustfs_utils::error_codes::ToErrorCode;

fn handle_error(error: &dyn ToErrorCode) {
    let code = error.to_error_code();
    
    log::error!(
        "Operation failed: code=0x{:08X}, type={}, message={}",
        code.as_u32(),
        code.error_type_name(),
        error
    );
    
    // 根据错误类型进行不同处理
    if code.is_storage_error() {
        handle_storage_error(code);
    } else if code.is_auth_error() {
        handle_auth_error(code);
    }
}
```

### 4. 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rustfs_utils::error_codes::{ErrorCode, error_types};

    #[test]
    fn test_error_code_conversion() {
        let error = Error::FileNotFound;
        let code = error.to_error_code();
        
        assert_eq!(code.error_type(), error_types::FILEMETA);
        assert_eq!(code.specific_code(), error_codes::FILE_NOT_FOUND);
        
        let recovered = Error::from_error_code(code);
        assert!(matches!(recovered, Some(Error::FileNotFound)));
    }
    
    #[test]
    fn test_error_code_uniqueness() {
        let filemeta_code = ErrorCode::new(error_types::FILEMETA, error_codes::FILE_NOT_FOUND);
        let storage_code = ErrorCode::new(error_types::STORAGE, error_codes::FILE_NOT_FOUND);
        
        // 即使使用相同的具体错误代码，完整的错误代码也应该不同
        assert_ne!(filemeta_code.as_u32(), storage_code.as_u32());
    }
}
```

## 错误监控和分析

### 1. 错误统计

```rust
use std::collections::HashMap;
use rustfs_utils::error_codes::ErrorCode;

pub struct ErrorStats {
    counts: HashMap<u32, u64>,
    type_counts: HashMap<u16, u64>,
}

impl ErrorStats {
    pub fn record_error(&mut self, code: ErrorCode) {
        *self.counts.entry(code.as_u32()).or_insert(0) += 1;
        *self.type_counts.entry(code.error_type()).or_insert(0) += 1;
    }
    
    pub fn get_top_errors(&self, limit: usize) -> Vec<(ErrorCode, u64)> {
        let mut errors: Vec<_> = self.counts.iter()
            .map(|(&code, &count)| (ErrorCode::from_u32(code), count))
            .collect();
        errors.sort_by(|a, b| b.1.cmp(&a.1));
        errors.truncate(limit);
        errors
    }
    
    pub fn get_error_distribution(&self) -> Vec<(String, u64)> {
        self.type_counts.iter()
            .map(|(&type_code, &count)| {
                let type_name = ErrorCode::new(type_code, 0).error_type_name();
                (type_name.to_string(), count)
            })
            .collect()
    }
}
```

### 2. 错误分析工具

```rust
pub fn analyze_error_patterns(errors: &[ErrorCode]) {
    let mut analysis = HashMap::new();
    
    for error in errors {
        let key = (error.error_type(), error.specific_code());
        *analysis.entry(key).or_insert(0) += 1;
    }
    
    println!("Error Pattern Analysis:");
    for ((error_type, specific_code), count) in analysis {
        let type_name = ErrorCode::new(error_type, 0).error_type_name();
        println!("  {}:0x{:04X} - {} occurrences", type_name, specific_code, count);
    }
}
```

## 迁移指南

### 从旧系统迁移

1. **更新依赖**：将 `rustfs-common` 替换为 `rustfs-utils`
2. **重新定义错误代码**：使用新的 `error_codes` 模块和 `common_ranges`
3. **实现转换 traits**：为每个错误类型实现 `ToErrorCode` 和 `FromErrorCode`
4. **更新测试**：确保错误代码转换正确工作

### 新模块开发

1. **选择错误类型**：从 `error_types` 中选择合适的错误类型
2. **定义错误代码**：优先使用 `common_ranges`，必要时定义模块特定代码
3. **实现转换**：实现 `ToErrorCode` 和 `FromErrorCode` traits
4. **添加测试**：确保错误代码系统正常工作

## 总结

新的 RustFS 错误代码系统提供了：

1. **统一的错误类型定义**：在 `rustfs-utils` 中集中管理
2. **分布式的错误实现**：各模块自己实现具体错误代码
3. **通用的错误代码范围**：提供一致的错误代码使用模式
4. **灵活的扩展能力**：支持新的错误类型和模块特定错误
5. **强类型安全**：通过 traits 确保错误转换的正确性

这个系统为 RustFS 项目提供了一个可扩展、可维护且类型安全的错误处理基础设施。 