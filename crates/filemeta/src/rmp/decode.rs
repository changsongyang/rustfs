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

use std::str::from_utf8;

use num_traits::cast::FromPrimitive;
use rmp::Marker;
use rmp::decode::{DecodeStringError, MarkerReadError, NumValueReadError, RmpReadErr, ValueReadError};

macro_rules! read_byteorder_utils {
    ($($name:ident => $tp:ident),* $(,)?) => {
        $(
            #[inline]
            #[doc(hidden)]
            fn $name(&mut self) -> impl std::future::Future<Output = Result<$tp, ValueReadError<Self::Error>>> + Send where Self: Sized {async {
                const SIZE: usize = core::mem::size_of::<$tp>();
                let mut buf: [u8; SIZE] = [0u8; SIZE];
                self.read_exact_buf(&mut buf).await.map_err(ValueReadError::InvalidDataRead)?;
                Ok(paste::paste! {
                    <byteorder::BigEndian as byteorder::ByteOrder>::[<read_ $tp>](&mut buf)
                })
            } }
        )*
    };
}

#[async_trait::async_trait]
pub trait RmpReader: Send + Sync {
    type Error: RmpReadErr;

    async fn read_exact_buf(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;

    async fn read_u8(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0; 1];
        self.read_exact_buf(&mut buf).await?;
        Ok(buf[0])
    }

    async fn read_data_u8(&mut self) -> Result<u8, ValueReadError<Self::Error>> {
        self.read_u8().await.map_err(ValueReadError::InvalidDataRead)
    }

    async fn read_data_i8(&mut self) -> Result<i8, ValueReadError<Self::Error>> {
        self.read_data_u8().await.map(|v| v as i8)
    }

    read_byteorder_utils!(
        read_data_u16 => u16,
        read_data_u32 => u32,
        read_data_u64 => u64,
        read_data_i16 => i16,
        read_data_i32 => i32,
        read_data_i64 => i64,
        read_data_f32 => f32,
        read_data_f64 => f64
    );
}

#[async_trait::async_trait]
impl<T: tokio::io::AsyncRead + Unpin + Send + Sync> RmpReader for T {
    type Error = std::io::Error;

    async fn read_exact_buf(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncReadExt::read_exact(self, buf).await?;
        Ok(())
    }
}

pub async fn read_marker<R: RmpReader>(rd: &mut R) -> Result<Marker, MarkerReadError<R::Error>> {
    let marker = rd.read_u8().await?;
    Ok(Marker::from_u8(marker))
}

pub async fn read_nil<R: RmpReader>(rd: &mut R) -> Result<(), ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::Null => Ok(()),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 9 bytes from the given reader and to decode them as integral `T` value.
///
/// This function will try to read up to 9 bytes from the reader (1 for marker and up to 8 for data)
/// and interpret them as a big-endian `T`.
///
/// Unlike `read_*`, this function weakens type restrictions, allowing you to safely decode packed
/// values even if you aren't sure about the actual integral type.
///
/// # Errors
///
/// This function will return `NumValueReadError` on any I/O error while reading either the marker
/// or the data.
///
/// It also returns `NumValueReadError::OutOfRange` if the actual type is not an integer or it does
/// not fit in the given numeric range.
///
/// # Examples
///
/// ```
/// let buf = [0xcd, 0x1, 0x2c];
///
/// assert_eq!(300u16, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300i16, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300u32, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300i32, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300u64, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300i64, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300usize, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// assert_eq!(300isize, rmp::decode::read_int(&mut &buf[..]).unwrap());
/// ```
pub async fn read_int<T: FromPrimitive, R: RmpReader>(rd: &mut R) -> Result<T, NumValueReadError<R::Error>> {
    let val = match read_marker(rd).await? {
        Marker::FixPos(val) => T::from_u8(val),
        Marker::FixNeg(val) => T::from_i8(val),
        Marker::U8 => T::from_u8(rd.read_data_u8().await?),
        Marker::U16 => T::from_u16(rd.read_data_u16().await?),
        Marker::U32 => T::from_u32(rd.read_data_u32().await?),
        Marker::U64 => T::from_u64(rd.read_data_u64().await?),
        Marker::I8 => T::from_i8(rd.read_data_i8().await?),
        Marker::I16 => T::from_i16(rd.read_data_i16().await?),
        Marker::I32 => T::from_i32(rd.read_data_i32().await?),
        Marker::I64 => T::from_i64(rd.read_data_i64().await?),
        marker => return Err(NumValueReadError::TypeMismatch(marker)),
    };

    val.ok_or(NumValueReadError::OutOfRange)
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as a big-endian u32
/// array size.
///
/// Array format family stores a sequence of elements in 1, 3, or 5 bytes of extra bytes in addition
/// to the elements.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
// TODO: Docs.
// NOTE: EINTR is managed internally.
pub async fn read_array_len<R>(rd: &mut R) -> Result<u32, ValueReadError<R::Error>>
where
    R: RmpReader,
{
    match read_marker(rd).await? {
        Marker::FixArray(size) => Ok(u32::from(size)),
        Marker::Array16 => Ok(u32::from(rd.read_data_u16().await?)),
        Marker::Array32 => Ok(rd.read_data_u32().await?),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as a big-endian u32
/// map size.
///
/// Map format family stores a sequence of elements in 1, 3, or 5 bytes of extra bytes in addition
/// to the elements.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
// TODO: Docs.
pub async fn read_map_len<R: RmpReader>(rd: &mut R) -> Result<u32, ValueReadError<R::Error>> {
    let marker = read_marker(rd).await?;
    marker_to_len(rd, marker).await
}

pub async fn marker_to_len<R: RmpReader>(rd: &mut R, marker: Marker) -> Result<u32, ValueReadError<R::Error>> {
    match marker {
        Marker::FixMap(size) => Ok(u32::from(size)),
        Marker::Map16 => Ok(u32::from(rd.read_data_u16().await?)),
        Marker::Map32 => Ok(rd.read_data_u32().await?),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read up to 5 bytes from the given reader and to decode them as Binary array length.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
// TODO: Docs.
pub async fn read_bin_len<R: RmpReader>(rd: &mut R) -> Result<u32, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::Bin8 => Ok(u32::from(rd.read_data_u8().await?)),
        Marker::Bin16 => Ok(u32::from(rd.read_data_u16().await?)),
        Marker::Bin32 => Ok(rd.read_data_u32().await?),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

#[inline]
pub async fn read_str_len<R: RmpReader>(rd: &mut R) -> Result<u32, ValueReadError<R::Error>> {
    Ok(read_str_len_with_nread(rd).await?.0)
}

async fn read_str_len_with_nread<R>(rd: &mut R) -> Result<(u32, usize), ValueReadError<R::Error>>
where
    R: RmpReader,
{
    match read_marker(rd).await? {
        Marker::FixStr(size) => Ok((u32::from(size), 1)),
        Marker::Str8 => Ok((u32::from(rd.read_data_u8().await?), 2)),
        Marker::Str16 => Ok((u32::from(rd.read_data_u16().await?), 3)),
        Marker::Str32 => Ok((rd.read_data_u32().await?, 5)),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

pub async fn read_str<'r, R>(rd: &mut R, buf: &'r mut [u8]) -> Result<&'r str, DecodeStringError<'r, R::Error>>
where
    R: RmpReader,
{
    let len = read_str_len(rd).await?;
    let ulen = len as usize;

    if buf.len() < ulen {
        return Err(DecodeStringError::BufferSizeTooSmall(len));
    }

    read_str_data(rd, len, &mut buf[0..ulen]).await
}

pub async fn read_str_data<'r, R>(rd: &mut R, len: u32, buf: &'r mut [u8]) -> Result<&'r str, DecodeStringError<'r, R::Error>>
where
    R: RmpReader,
{
    debug_assert_eq!(len as usize, buf.len());

    // Trying to copy exact `len` bytes.
    match rd.read_exact_buf(buf).await {
        Ok(()) => match from_utf8(buf) {
            Ok(decoded) => Ok(decoded),
            Err(err) => Err(DecodeStringError::InvalidUtf8(buf, err)),
        },
        Err(err) => Err(DecodeStringError::InvalidDataRead(err)),
    }
}

/// Attempts to read a single byte from the given reader and to decode it as a positive fixnum
/// value.
///
/// According to the MessagePack specification, a positive fixed integer value is represented using
/// a single byte in `[0x00; 0x7f]` range inclusively, prepended with a special marker mask.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading the marker,
/// except the EINTR, which is handled internally.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub async fn read_pfix<R: RmpReader>(rd: &mut R) -> Result<u8, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::FixPos(val) => Ok(val),
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 2 bytes from the given reader and to decode them as `u8` value.
///
/// The first byte should be the marker and the second one should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
pub async fn read_u8<R: RmpReader>(rd: &mut R) -> Result<u8, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::U8 => rd.read_data_u8().await,
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 3 bytes from the given reader and to decode them as `u16` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub async fn read_u16<R: RmpReader>(rd: &mut R) -> Result<u16, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::U16 => rd.read_data_u16().await,
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 5 bytes from the given reader and to decode them as `u32` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub async fn read_u32<R: RmpReader>(rd: &mut R) -> Result<u32, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::U32 => rd.read_data_u32().await,
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}

/// Attempts to read exactly 9 bytes from the given reader and to decode them as `u64` value.
///
/// The first byte should be the marker and the others should represent the data itself.
///
/// # Errors
///
/// This function will return `ValueReadError` on any I/O error while reading either the marker or
/// the data.
///
/// It also returns `ValueReadError::TypeMismatch` if the actual type is not equal with the
/// expected one, indicating you with the actual type.
///
/// # Note
///
/// This function will silently retry on every EINTR received from the underlying `Read` until
/// successful read.
pub async fn read_u64<R: RmpReader>(rd: &mut R) -> Result<u64, ValueReadError<R::Error>> {
    match read_marker(rd).await? {
        Marker::U64 => rd.read_data_u64().await,
        marker => Err(ValueReadError::TypeMismatch(marker)),
    }
}
