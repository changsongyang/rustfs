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

use rmp::{
    Marker,
    encode::{DataWriteError, RmpWriteErr, ValueWriteError},
};

macro_rules! write_byteorder_utils {
    ($($name:ident => $tp:ident),* $(,)?) => {
        $(
            #[inline]
            #[doc(hidden)]
            fn $name(&mut self, val: $tp) -> impl std::future::Future<Output = Result<(), DataWriteError<Self::Error>>> + Send {async move {
                const SIZE: usize = core::mem::size_of::<$tp>();
                let mut buf: [u8; SIZE] = [0u8; SIZE];
                paste::paste! {
                    <byteorder::BigEndian as byteorder::ByteOrder>::[<write_ $tp>](&mut buf, val);
                }
                self.write_bytes(&buf).await.map_err(DataWriteError::from)
            } }
        )*
    };
}

/// A type that `rmp` supports writing into.
///
/// The methods of this trait should be considered an implementation detail (for now).
/// It is currently sealed (can not be implemented by the user).
///
/// See also [`std::io::Write`] and [`byteorder::WriteBytesExt`]
///
/// Its primary implementations are [`std::io::Write`] and [`ByteBuf`].
#[async_trait::async_trait]
pub trait RmpWriter: Send {
    type Error: RmpWriteErr;

    /// Write a single byte to this stream
    #[inline]
    async fn write_u8(&mut self, val: u8) -> Result<(), Self::Error> {
        let buf = [val];
        self.write_bytes(&buf).await
    }

    /// Write a slice of bytes to the underlying stream
    ///
    /// This will either write all the bytes or return an error.
    /// See also [`std::io::Write::write_all`]
    async fn write_bytes(&mut self, buf: &[u8]) -> Result<(), Self::Error>;

    // Internal helper functions to map I/O error into the `DataWriteError` error.

    /// Write a single (signed) byte to this stream.
    #[inline]
    #[doc(hidden)]
    async fn write_data_u8(&mut self, val: u8) -> Result<(), DataWriteError<Self::Error>> {
        self.write_u8(val).await.map_err(DataWriteError::from)
    }
    /// Write a single (signed) byte to this stream.
    #[inline]
    #[doc(hidden)]
    async fn write_data_i8(&mut self, val: i8) -> Result<(), DataWriteError<Self::Error>> {
        self.write_data_u8(val as u8).await
    }

    write_byteorder_utils!(
        write_data_u16 => u16,
        write_data_u32 => u32,
        write_data_u64 => u64,
        write_data_i16 => i16,
        write_data_i32 => i32,
        write_data_i64 => i64,
        write_data_f32 => f32,
        write_data_f64 => f64
    );
}

#[async_trait::async_trait]
impl<T: tokio::io::AsyncWrite + Unpin + Send + Sync> RmpWriter for T {
    type Error = std::io::Error;

    async fn write_bytes(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(self, buf).await?;
        Ok(())
    }
}

/// Attempts to write the given marker into the writer.
async fn write_marker<W: RmpWriter>(wr: &mut W, marker: Marker) -> Result<(), ValueWriteError<W::Error>> {
    wr.write_u8(marker.to_u8()).await.map_err(ValueWriteError::InvalidMarkerWrite)
}

/// Encodes and attempts to write the most efficient string length implementation to the given
/// write, returning the marker used.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_str_len<W: RmpWriter>(wr: &mut W, len: u32) -> Result<Marker, ValueWriteError<W::Error>> {
    let marker = if len < 32 {
        Marker::FixStr(len as u8)
    } else if len < 256 {
        Marker::Str8
    } else if len <= u16::MAX as u32 {
        Marker::Str16
    } else {
        Marker::Str32
    };

    write_marker(wr, marker).await?;
    if marker == Marker::Str8 {
        wr.write_data_u8(len as u8).await?;
    }
    if marker == Marker::Str16 {
        wr.write_data_u16(len as u16).await?;
    }
    if marker == Marker::Str32 {
        wr.write_data_u32(len).await?;
    }
    Ok(marker)
}

/// Encodes and attempts to write the most efficient string binary representation to the
/// given `Write`.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
// TODO: Docs, range check, example, visibility.
pub async fn write_str<W: RmpWriter>(wr: &mut W, data: &str) -> Result<(), ValueWriteError<W::Error>> {
    write_str_len(wr, data.len() as u32).await?;
    wr.write_bytes(data.as_bytes())
        .await
        .map_err(ValueWriteError::InvalidDataWrite)
}

/// Encodes and attempts to write an unsigned small integer value as a positive fixint into the
/// given write.
///
/// According to the MessagePack specification, a positive fixed integer value is represented using
/// a single byte in `[0x00; 0x7f]` range inclusively, prepended with a special marker mask.
///
/// The function is **strict** with the input arguments - it is the user's responsibility to check
/// if the value fits in the described range, otherwise it will panic.
///
/// If you are not sure if the value fits in the given range use `write_uint` instead, which
/// automatically selects the most compact integer representation.
///
/// # Errors
///
/// This function will return `FixedValueWriteError` on any I/O error occurred while writing the
/// positive integer marker.
///
/// # Panics
///
/// Panics if `val` is greater than 127.
#[inline]
pub async fn write_pfix<W: RmpWriter>(wr: &mut W, val: u8) -> Result<(), ValueWriteError<W::Error>> {
    assert!(val < 128);
    write_marker(wr, Marker::FixPos(val)).await?;
    Ok(())
}

/// Encodes and attempts to write an `u8` value as a 2-byte sequence into the given write.
///
/// The first byte becomes the marker and the second one will represent the data itself.
///
/// Note, that this function will encode the given value in 2-byte sequence no matter what, even if
/// the value can be represented using single byte as a positive fixnum.
///
/// If you need to fit the given buffer efficiently use `write_uint` instead, which automatically
/// selects the appropriate integer representation.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
///
/// # Examples
/// ```
/// let mut buf = [0x00, 0x00];
///
/// rmp::encode::write_u8(&mut &mut buf[..], 146).ok().unwrap();
/// assert_eq!([0xcc, 0x92], buf);
///
/// // Note, that 42 can be represented simply as `[0x2a]`, but the function emits 2-byte sequence.
/// rmp::encode::write_u8(&mut &mut buf[..], 42).ok().unwrap();
/// assert_eq!([0xcc, 0x2a], buf);
/// ```
pub async fn write_u8<W: RmpWriter>(wr: &mut W, val: u8) -> Result<(), ValueWriteError<W::Error>> {
    write_marker(wr, Marker::U8).await?;
    wr.write_data_u8(val).await?;
    Ok(())
}

/// Encodes and attempts to write an `u16` value strictly as a 3-byte sequence into the given write.
///
/// The first byte becomes the marker and the others will represent the data itself.
///
/// Note, that this function will encode the given value in 3-byte sequence no matter what, even if
/// the value can be represented using single byte as a positive fixnum.
///
/// If you need to fit the given buffer efficiently use `write_uint` instead, which automatically
/// selects the appropriate integer representation.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_u16<W: RmpWriter>(wr: &mut W, val: u16) -> Result<(), ValueWriteError<W::Error>> {
    write_marker(wr, Marker::U16).await?;
    wr.write_data_u16(val).await?;
    Ok(())
}

/// Encodes and attempts to write an `u32` value strictly as a 5-byte sequence into the given write.
///
/// The first byte becomes the marker and the others will represent the data itself.
///
/// Note, that this function will encode the given value in 5-byte sequence no matter what, even if
/// the value can be represented using single byte as a positive fixnum.
///
/// If you need to fit the given buffer efficiently use `write_uint` instead, which automatically
/// selects the appropriate integer representation.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_u32<W: RmpWriter>(wr: &mut W, val: u32) -> Result<(), ValueWriteError<W::Error>> {
    write_marker(wr, Marker::U32).await?;
    wr.write_data_u32(val).await?;
    Ok(())
}

/// Encodes and attempts to write an `u64` value strictly as a 9-byte sequence into the given write.
///
/// The first byte becomes the marker and the others will represent the data itself.
///
/// Note, that this function will encode the given value in 9-byte sequence no matter what, even if
/// the value can be represented using single byte as a positive fixnum.
///
/// If you need to fit the given buffer efficiently use `write_uint` instead, which automatically
/// selects the appropriate integer representation.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_u64<W: RmpWriter>(wr: &mut W, val: u64) -> Result<(), ValueWriteError<W::Error>> {
    write_marker(wr, Marker::U64).await?;
    wr.write_data_u64(val).await?;
    Ok(())
}

/// Encodes and attempts to write an `u8` value into the given write using the most efficient
/// representation, returning the marker used.
///
/// See [`write_uint`] for more info.
pub async fn write_uint8<W: RmpWriter>(wr: &mut W, val: u8) -> Result<Marker, ValueWriteError<W::Error>> {
    if val < 128 {
        write_pfix(wr, val).await?;
        Ok(Marker::FixPos(val))
    } else {
        write_u8(wr, val).await?;
        Ok(Marker::U8)
    }
}

/// Encodes and attempts to write an `u64` value into the given write using the most efficient
/// representation, returning the marker used.
///
/// This function obeys the MessagePack specification, which requires that the serializer SHOULD use
/// the format which represents the data in the smallest number of bytes.
///
/// The first byte becomes the marker and the others (if present, up to 9) will represent the data
/// itself.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_uint<W: RmpWriter>(wr: &mut W, val: u64) -> Result<Marker, ValueWriteError<W::Error>> {
    if val < 256 {
        write_uint8(wr, val as u8).await
    } else if val < 65536 {
        write_u16(wr, val as u16).await?;
        Ok(Marker::U16)
    } else if val < 4294967296 {
        write_u32(wr, val as u32).await?;
        Ok(Marker::U32)
    } else {
        write_u64(wr, val).await?;
        Ok(Marker::U64)
    }
}

/// Encodes and attempts to write the most efficient binary array length implementation to the given
/// write, returning the marker used.
///
/// This function is useful when you want to get full control for writing the data itself, for
/// example, when using non-blocking socket.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub async fn write_bin_len<W: RmpWriter>(wr: &mut W, len: u32) -> Result<Marker, ValueWriteError<W::Error>> {
    let marker = if len < 256 {
        Marker::Bin8
    } else if len <= u16::MAX as u32 {
        Marker::Bin16
    } else {
        Marker::Bin32
    };
    write_marker(&mut *wr, marker).await?;
    if marker == Marker::Bin8 {
        wr.write_data_u8(len as u8).await?;
    } else if marker == Marker::Bin16 {
        wr.write_data_u16(len as u16).await?;
    } else if marker == Marker::Bin32 {
        wr.write_data_u32(len).await?;
    }
    Ok(marker)
}

/// Encodes and attempts to write the most efficient binary implementation to the given `Write`.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
// TODO: Docs, range check, example, visibility.
pub async fn write_bin<W: RmpWriter>(wr: &mut W, data: &[u8]) -> Result<(), ValueWriteError<W::Error>> {
    write_bin_len(wr, data.len() as u32).await?;
    wr.write_bytes(data).await.map_err(ValueWriteError::InvalidDataWrite)
}
