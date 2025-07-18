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

use crate::disk::error::{Error, Result};
use rustfs_filemeta::rmp::{self, RmpReader, RmpWriter};
use rustfs_filemeta::{FileInfo, FileInfoVersions, FileMeta, FileMetaShallowVersion, VersionType, merge_file_meta_versions};
use rustfs_utils::error_codes::{AutoErrorCode as _, ToErrorCode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt::Debug;
use time::OffsetDateTime;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::warn;

const SLASH_SEPARATOR: &str = "/";

#[derive(Clone, Debug, Default)]
pub struct MetadataResolutionParams {
    pub dir_quorum: usize,
    pub obj_quorum: usize,
    pub requested_versions: usize,
    pub bucket: String,
    pub strict: bool,
    pub candidates: Vec<Vec<FileMetaShallowVersion>>,
}

/// MetaCacheEntryType is the type of the entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum MetaCacheEntryType {
    /// Object is a valid object.
    Object,
    /// Error is an error.
    Error,
    /// Close is a close message.
    #[default]
    Close,
}

impl MetaCacheEntryType {
    pub fn to_u8(&self) -> u8 {
        match self {
            MetaCacheEntryType::Object => 1,
            MetaCacheEntryType::Error => 2,
            MetaCacheEntryType::Close => 0,
        }
    }

    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => MetaCacheEntryType::Object,
            2 => MetaCacheEntryType::Error,
            0 => MetaCacheEntryType::Close,
            _ => MetaCacheEntryType::Close, // default to close
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MetaCacheEntry {
    /// msg_type is the type of the entry.
    pub msg_type: MetaCacheEntryType,

    /// name is the full name of the object including prefixes
    pub name: String,
    /// Metadata. If none is present it is not an object but only a prefix.
    /// Entries without metadata will only be present in non-recursive scans.
    pub metadata: Vec<u8>,

    #[serde(skip)]
    pub error: Option<Error>,

    /// cached contains the metadata if decoded.
    #[serde(skip)]
    pub cached: Option<FileMeta>,

    /// Indicates the entry can be reused and only one reference to metadata is expected.
    #[serde(skip)]
    pub reusable: bool,
}

impl MetaCacheEntry {
    pub async fn write_to<W: RmpWriter>(&self, w: &mut W) -> Result<()> {
        rmp::write_pfix(w, self.msg_type.to_u8())
            .await
            .map_err(|e| Error::other(e.to_string()))?;
        rmp::write_str(w, &self.name).await.map_err(|e| Error::other(e.to_string()))?;
        rmp::write_bin(w, &self.metadata)
            .await
            .map_err(|e| Error::other(e.to_string()))?;

        let (err_no, err_msg) = match &self.error {
            Some(err) => (err.code(), err.to_string()),
            None => (0, "".to_owned()),
        };

        rmp::write_u32(w, err_no).await.map_err(|e| Error::other(e.to_string()))?;
        rmp::write_str(w, &err_msg).await.map_err(|e| Error::other(e.to_string()))?;
        Ok(())
    }

    pub async fn read_from<R: RmpReader>(rd: &mut R) -> Result<Self> {
        let msg_type = rmp::read_pfix(rd).await.map_err(|e| Error::other(format!("{e:?}")))?;

        let name_len = rmp::read_str_len(rd).await.map_err(|e| Error::other(format!("{e:?}")))?;
        let mut name_buf = vec![0; name_len as usize];
        let name = rmp::read_str_data(rd, name_len, &mut name_buf)
            .await
            .map_err(|e| Error::other(e.to_string()))
            .map(|s| s.to_owned())?;

        let metadata_len = rmp::read_bin_len(rd).await.map_err(|e| Error::other(format!("{e:?}")))?;
        let mut metadata = vec![0; metadata_len as usize];
        rmp::read_bytes_data(rd, metadata_len, &mut metadata)
            .await
            .map_err(|e| Error::other(e.to_string()))?;

        let err_no = rmp::read_u32(rd).await.map_err(|e| Error::other(format!("{e:?}")))?;

        let err_len = rmp::read_str_len(rd).await.map_err(|e| Error::other(format!("{e:?}")))?;
        let mut err_buf = vec![0; err_len as usize];
        let err_msg = rmp::read_str_data(rd, err_len, &mut err_buf)
            .await
            .map_err(|e| Error::other(e.to_string()))
            .map(|s| s.to_owned())?;

        let error = Error::from_code(err_no).map(|v| if matches!(v, Error::Io(_)) { Error::other(err_msg) } else { v });

        Ok(Self {
            msg_type: MetaCacheEntryType::from_u8(msg_type),
            name,
            metadata,
            error,
            cached: None,
            reusable: false,
        })
    }

    pub fn is_dir(&self) -> bool {
        self.metadata.is_empty() && self.name.ends_with('/')
    }

    pub fn is_in_dir(&self, dir: &str, separator: &str) -> bool {
        if dir.is_empty() {
            let idx = self.name.find(separator);
            return idx.is_none() || idx.unwrap() == self.name.len() - separator.len();
        }

        let ext = self.name.trim_start_matches(dir);

        if ext.len() != self.name.len() {
            let idx = ext.find(separator);
            return idx.is_none() || idx.unwrap() == ext.len() - separator.len();
        }

        false
    }

    pub fn is_object(&self) -> bool {
        !self.metadata.is_empty()
    }

    pub fn is_object_dir(&self) -> bool {
        !self.metadata.is_empty() && self.name.ends_with(SLASH_SEPARATOR)
    }

    pub fn is_latest_delete_marker(&mut self) -> bool {
        if let Some(cached) = &self.cached {
            if cached.versions.is_empty() {
                return true;
            }
            return cached.versions[0].header.version_type == VersionType::Delete;
        }

        if !FileMeta::is_xl2_v1_format(&self.metadata) {
            return false;
        }

        match FileMeta::check_xl2_v1(&self.metadata) {
            Ok((meta, _, _)) => {
                if !meta.is_empty() {
                    return FileMeta::is_latest_delete_marker(meta);
                }
            }
            Err(_) => return true,
        }

        match self.xl_meta() {
            Some(res) => {
                if res.versions.is_empty() {
                    return true;
                }
                res.versions[0].header.version_type == VersionType::Delete
            }
            None => true,
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub fn to_fileinfo(&self, bucket: &str) -> Result<FileInfo> {
        if self.is_dir() {
            return Ok(FileInfo {
                volume: bucket.to_owned(),
                name: self.name.clone(),
                ..Default::default()
            });
        }

        if self.cached.is_some() {
            let fm = self.cached.as_ref().unwrap();
            if fm.versions.is_empty() {
                return Ok(FileInfo {
                    volume: bucket.to_owned(),
                    name: self.name.clone(),
                    deleted: true,
                    is_latest: true,
                    mod_time: Some(OffsetDateTime::UNIX_EPOCH),
                    ..Default::default()
                });
            }

            let fi = fm.into_fileinfo(bucket, self.name.as_str(), "", false, false)?;
            return Ok(fi);
        }

        let mut fm = FileMeta::new();
        fm.unmarshal_msg(&self.metadata)?;
        let fi = fm.into_fileinfo(bucket, self.name.as_str(), "", false, false)?;
        Ok(fi)
    }

    pub fn file_info_versions(&self, bucket: &str) -> Result<FileInfoVersions> {
        if self.is_dir() {
            return Ok(FileInfoVersions {
                volume: bucket.to_string(),
                name: self.name.clone(),
                versions: vec![FileInfo {
                    volume: bucket.to_string(),
                    name: self.name.clone(),
                    ..Default::default()
                }],
                ..Default::default()
            });
        }

        let mut fm = FileMeta::new();
        fm.unmarshal_msg(&self.metadata)?;
        fm.into_file_info_versions(bucket, self.name.as_str(), false)
            .map_err(|e| e.into())
    }

    pub fn matches(&self, other: Option<&MetaCacheEntry>, strict: bool) -> (Option<MetaCacheEntry>, bool) {
        if other.is_none() {
            return (None, false);
        }

        let other = other.unwrap();
        if self.name != other.name {
            if self.name < other.name {
                return (Some(self.clone()), false);
            }
            return (Some(other.clone()), false);
        }

        if other.is_dir() || self.is_dir() {
            if self.is_dir() {
                return (Some(self.clone()), other.is_dir() == self.is_dir());
            }
            return (Some(other.clone()), other.is_dir() == self.is_dir());
        }

        let self_vers = match &self.cached {
            Some(file_meta) => file_meta.clone(),
            None => match FileMeta::load(&self.metadata) {
                Ok(meta) => meta,
                Err(_) => return (None, false),
            },
        };

        let other_vers = match &other.cached {
            Some(file_meta) => file_meta.clone(),
            None => match FileMeta::load(&other.metadata) {
                Ok(meta) => meta,
                Err(_) => return (None, false),
            },
        };

        if self_vers.versions.len() != other_vers.versions.len() {
            match self_vers.lastest_mod_time().cmp(&other_vers.lastest_mod_time()) {
                Ordering::Greater => return (Some(self.clone()), false),
                Ordering::Less => return (Some(other.clone()), false),
                _ => {}
            }

            if self_vers.versions.len() > other_vers.versions.len() {
                return (Some(self.clone()), false);
            }
            return (Some(other.clone()), false);
        }

        let mut prefer = None;
        for (s_version, o_version) in self_vers.versions.iter().zip(other_vers.versions.iter()) {
            if s_version.header != o_version.header {
                if s_version.header.has_ec() != o_version.header.has_ec() {
                    // One version has EC and the other doesn't - may have been written later.
                    // Compare without considering EC.
                    let (mut a, mut b) = (s_version.header.clone(), o_version.header.clone());
                    (a.ec_n, a.ec_m, b.ec_n, b.ec_m) = (0, 0, 0, 0);
                    if a == b {
                        continue;
                    }
                }

                if !strict && s_version.header.matches_not_strict(&o_version.header) {
                    if prefer.is_none() {
                        if s_version.header.sorts_before(&o_version.header) {
                            prefer = Some(self.clone());
                        } else {
                            prefer = Some(other.clone());
                        }
                    }
                    continue;
                }

                if prefer.is_some() {
                    return (prefer, false);
                }

                if s_version.header.sorts_before(&o_version.header) {
                    return (Some(self.clone()), false);
                }

                return (Some(other.clone()), false);
            }
        }

        if prefer.is_none() {
            prefer = Some(self.clone());
        }

        (prefer, true)
    }

    pub fn xl_meta(&mut self) -> Option<FileMeta> {
        if self.is_dir() {
            return None;
        }

        if let Some(meta) = &self.cached {
            Some(meta.clone())
        } else {
            if self.metadata.is_empty() {
                return None;
            }

            let meta = FileMeta::load(&self.metadata).ok()?;
            self.cached = Some(meta.clone());
            Some(meta)
        }
    }
}

#[derive(Debug, Default)]
pub struct MetaCacheEntries(pub Vec<Option<MetaCacheEntry>>);

impl MetaCacheEntries {
    #[allow(clippy::should_implement_trait)]
    pub fn as_ref(&self) -> &[Option<MetaCacheEntry>] {
        &self.0
    }

    pub fn resolve(&self, mut params: MetadataResolutionParams) -> Option<MetaCacheEntry> {
        if self.0.is_empty() {
            warn!("decommission_pool: entries resolve empty");
            return None;
        }

        let mut dir_exists = 0;
        let mut selected = None;

        params.candidates.clear();
        let mut objs_agree = 0;
        let mut objs_valid = 0;

        for entry in self.0.iter().flatten() {
            let mut entry = entry.clone();

            warn!("decommission_pool: entries resolve entry {:?}", entry.name);
            if entry.name.is_empty() {
                continue;
            }
            if entry.is_dir() {
                dir_exists += 1;
                selected = Some(entry.clone());
                warn!("decommission_pool: entries resolve entry dir {:?}", entry.name);
                continue;
            }

            let xl = match entry.xl_meta() {
                Some(xl) => xl,
                None => {
                    warn!("decommission_pool: entries resolve entry xl_meta not found");
                    continue;
                }
            };

            objs_valid += 1;
            params.candidates.push(xl.versions.clone());

            if selected.is_none() {
                selected = Some(entry.clone());
                objs_agree = 1;
                warn!("decommission_pool: entries resolve entry selected {:?}", entry.name);
                continue;
            }

            if let (prefer, true) = entry.matches(selected.as_ref(), params.strict) {
                selected = prefer;
                objs_agree += 1;
                warn!("decommission_pool: entries resolve entry prefer {:?}", entry.name);
                continue;
            }
        }

        let Some(selected) = selected else {
            warn!("decommission_pool: entries resolve entry no selected");
            return None;
        };

        if selected.is_dir() && dir_exists >= params.dir_quorum {
            warn!("decommission_pool: entries resolve entry dir selected {:?}", selected.name);
            return Some(selected);
        }

        // If we would never be able to reach read quorum.
        if objs_valid < params.obj_quorum {
            warn!(
                "decommission_pool: entries resolve entry not enough objects {} < {}",
                objs_valid, params.obj_quorum
            );
            return None;
        }

        if objs_agree == objs_valid {
            warn!("decommission_pool: entries resolve entry all agree {} == {}", objs_agree, objs_valid);
            return Some(selected);
        }

        let Some(cached) = selected.cached else {
            warn!("decommission_pool: entries resolve entry no cached");
            return None;
        };

        let versions = merge_file_meta_versions(params.obj_quorum, params.strict, params.requested_versions, &params.candidates);
        if versions.is_empty() {
            warn!("decommission_pool: entries resolve entry no versions");
            return None;
        }

        let metadata = match cached.marshal_msg() {
            Ok(meta) => meta,
            Err(e) => {
                warn!("decommission_pool: entries resolve entry marshal_msg {:?}", e);
                return None;
            }
        };

        // Merge if we have disagreement.
        // Create a new merged result.
        let new_selected = MetaCacheEntry {
            name: selected.name.clone(),
            cached: Some(FileMeta {
                meta_ver: cached.meta_ver,
                versions,
                ..Default::default()
            }),
            reusable: true,
            metadata,
            msg_type: MetaCacheEntryType::Object,
            ..Default::default()
        };

        warn!("decommission_pool: entries resolve entry selected {:?}", new_selected.name);
        Some(new_selected)
    }

    pub fn first_found(&self) -> (Option<MetaCacheEntry>, usize) {
        (self.0.iter().find(|x| x.is_some()).cloned().unwrap_or_default(), self.0.len())
    }
}

#[derive(Debug, Default)]
pub struct MetaCacheEntriesSortedResult {
    pub entries: Option<MetaCacheEntriesSorted>,
    pub err: Option<Error>,
}

#[derive(Debug, Default)]
pub struct MetaCacheEntriesSorted {
    pub o: MetaCacheEntries,
    pub list_id: Option<String>,
    pub reuse: bool,
    pub last_skipped_entry: Option<String>,
}

impl MetaCacheEntriesSorted {
    pub fn entries(&self) -> Vec<&MetaCacheEntry> {
        let entries: Vec<&MetaCacheEntry> = self.o.0.iter().flatten().collect();
        entries
    }

    pub fn forward_past(&mut self, marker: Option<String>) {
        if let Some(val) = marker {
            if let Some(idx) = self.o.0.iter().flatten().position(|v| v.name > val) {
                self.o.0 = self.o.0.split_off(idx);
            }
        }
    }
}

const METACACHE_STREAM_VERSION_V1: u8 = 1;

#[derive(Debug)]
pub struct MetacacheWriter<W> {
    wr: W,
    created: bool,
}

#[async_trait::async_trait]
impl<W: AsyncWrite + Unpin + Send + Sync> RmpWriter for MetacacheWriter<W> {
    type Error = std::io::Error;

    async fn write_bytes(&mut self, buf: &[u8]) -> std::result::Result<(), Self::Error> {
        self.wr.write_all(buf).await?;
        Ok(())
    }
}

impl<W: AsyncWrite + Unpin + Send + Sync> MetacacheWriter<W> {
    pub fn new(wr: W) -> Self {
        Self { wr, created: false }
    }

    pub async fn init(&mut self) -> Result<()> {
        if !self.created {
            self.write_version(METACACHE_STREAM_VERSION_V1).await?;
            self.created = true;
        }
        Ok(())
    }

    pub async fn write(&mut self, objs: &[MetaCacheEntry]) -> Result<()> {
        if objs.is_empty() {
            return Ok(());
        }

        self.init().await?;

        for obj in objs.iter() {
            if obj.name.is_empty() {
                return Err(Error::other("metacacheWriter: no name"));
            }

            self.write_obj(obj).await?;
        }

        Ok(())
    }

    async fn write_version(&mut self, version: u8) -> Result<()> {
        rmp::write_pfix(&mut self.wr, version).await?;
        Ok(())
    }

    /// Write a single object to the buffer.
    pub async fn write_obj(&mut self, obj: &MetaCacheEntry) -> Result<()> {
        self.init().await?;

        obj.write_to(&mut self.wr).await?;

        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        let obj = MetaCacheEntry {
            msg_type: MetaCacheEntryType::Close,
            ..Default::default()
        };

        self.write_obj(&obj).await?;
        Ok(())
    }

    pub async fn write_err(&mut self, err_no: u32, err_msg: String) -> Result<()> {
        let obj = MetaCacheEntry {
            msg_type: MetaCacheEntryType::Error,
            err_no,
            err_msg,
            ..Default::default()
        };

        self.write_obj(&obj).await?;
        Ok(())
    }
}

pub struct MetacacheReader<R> {
    rd: R,
    init: bool,
    err: Option<Error>,
    current: Option<MetaCacheEntry>,
}

#[async_trait::async_trait]
impl<R: AsyncRead + Unpin + Send + Sync> RmpReader for MetacacheReader<R> {
    type Error = std::io::Error;

    async fn read_exact_buf(&mut self, buf: &mut [u8]) -> std::result::Result<(), Self::Error> {
        self.rd.read_exact(buf).await?;
        Ok(())
    }
}

impl<R: AsyncRead + Unpin + Send + Sync> MetacacheReader<R> {
    pub fn new(rd: R) -> Self {
        Self {
            rd,
            init: false,
            err: None,
            current: None,
        }
    }

    async fn read_version(&mut self) -> Result<u8> {
        rmp::read_pfix(&mut self.rd).await.map_err(Error::other)
    }

    async fn check_init(&mut self) -> Result<()> {
        if let Some(err) = &self.err {
            return Err(err.clone());
        }

        if self.init {
            return Ok(());
        }

        let ver = match self.read_version().await {
            Ok(ver) => ver,
            Err(e) => {
                self.err = Some(e.clone());
                return Err(e);
            }
        };
        match ver {
            METACACHE_STREAM_VERSION_V1 => (),
            _ => {
                self.err = Some(Error::other("invalid version"));
            }
        }

        self.init = true;

        if let Some(err) = &self.err {
            return Err(err.clone());
        }

        Ok(())
    }

    pub async fn skip(&mut self, size: usize) -> Result<()> {
        self.check_init().await?;

        let mut n = size;

        if self.current.is_some() {
            n -= 1;
            self.current = None;
        }

        while n > 0 {
            let entry = MetaCacheEntry::read_from(&mut self.rd).await?;
            if entry.msg_type == MetaCacheEntryType::Close {
                break;
            }

            if entry.msg_type == MetaCacheEntryType::Error {
                return Err(Error::other(entry.err_msg));
            }

            n -= 1;
        }

        Ok(())
    }

    pub async fn next(&mut self) -> Result<Option<MetaCacheEntry>> {
        self.check_init().await?;

        let entry = MetaCacheEntry::read_from(&mut self.rd).await?;

        if entry.msg_type == MetaCacheEntryType::Close {
            return Ok(None);
        }

        if entry.msg_type == MetaCacheEntryType::Error {
            return Err(Error::other(entry.err_msg));
        }

        Ok(Some(entry))
    }

    pub async fn read_all(&mut self) -> Result<Vec<MetaCacheEntry>> {
        let mut ret = Vec::new();
        self.check_init().await?;

        loop {
            // If we have a current entry, use it and clear it
            if let Some(entry) = self.current.take() {
                if entry.msg_type == MetaCacheEntryType::Close {
                    break;
                }
                if entry.msg_type == MetaCacheEntryType::Error {
                    return Err(Error::other(entry.err_msg));
                }
                ret.push(entry);
                continue;
            }

            // Read next entry
            let entry = MetaCacheEntry::read_from(&mut self.rd).await?;

            if entry.msg_type == MetaCacheEntryType::Close {
                break;
            }

            if entry.msg_type == MetaCacheEntryType::Error {
                return Err(Error::other(entry.err_msg));
            }

            ret.push(entry);
        }

        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_writer() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let mut objs = Vec::new();
        for i in 0..10 {
            let info = MetaCacheEntry {
                name: format!("item{i}"),
                metadata: vec![0u8, 10],
                cached: None,
                reusable: false,
                msg_type: MetaCacheEntryType::Object,
                err_no: 0,
                err_msg: String::new(),
            };
            objs.push(info);
        }

        w.write(&objs).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let nobjs = r.read_all().await.unwrap();

        assert_eq!(objs, nobjs);
    }

    #[tokio::test]
    async fn test_metacache_writer_empty_objects() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        // Test writing empty objects array
        let objs = Vec::new();
        w.write(&objs).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let nobjs = r.read_all().await.unwrap();

        assert_eq!(objs, nobjs);
    }

    #[tokio::test]
    async fn test_metacache_writer_single_object() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let obj = MetaCacheEntry {
            name: "test-object".to_string(),
            metadata: vec![1, 2, 3, 4, 5],
            cached: None,
            reusable: false,
            msg_type: MetaCacheEntryType::Object,
            err_no: 0,
            err_msg: String::new(),
        };

        w.write_obj(&obj).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let read_obj = r.next().await.unwrap().unwrap();

        assert_eq!(obj, read_obj);
    }

    #[tokio::test]
    async fn test_metacache_writer_error_entry() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let err_no = 404;
        let err_msg = "Object not found".to_string();

        w.write_err(err_no, err_msg.clone()).await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let result = r.next().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains(&err_msg));
    }

    #[tokio::test]
    async fn test_metacache_writer_directory_entry() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let dir_entry = MetaCacheEntry {
            name: "test-dir/".to_string(),
            metadata: Vec::new(), // Empty metadata indicates directory
            cached: None,
            reusable: false,
            msg_type: MetaCacheEntryType::Object,
            err_no: 0,
            err_msg: String::new(),
        };

        w.write_obj(&dir_entry).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let read_entry = r.next().await.unwrap().unwrap();

        assert_eq!(dir_entry, read_entry);
        assert!(read_entry.is_dir());
    }

    #[tokio::test]
    async fn test_metacache_writer_mixed_entries() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let entries = vec![
            MetaCacheEntry {
                name: "dir/".to_string(),
                metadata: Vec::new(),
                cached: None,
                reusable: false,
                msg_type: MetaCacheEntryType::Object,
                err_no: 0,
                err_msg: String::new(),
            },
            MetaCacheEntry {
                name: "file.txt".to_string(),
                metadata: vec![1, 2, 3],
                cached: None,
                reusable: false,
                msg_type: MetaCacheEntryType::Object,
                err_no: 0,
                err_msg: String::new(),
            },
        ];

        w.write(&entries).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let read_entries = r.read_all().await.unwrap();

        assert_eq!(entries.len(), read_entries.len());
        for (expected, actual) in entries.iter().zip(read_entries.iter()) {
            assert_eq!(expected, actual);
        }
    }

    #[tokio::test]
    async fn test_metacache_reader_skip() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let mut objs = Vec::new();
        for i in 0..5 {
            let info = MetaCacheEntry {
                name: format!("item{i}"),
                metadata: vec![i as u8],
                cached: None,
                reusable: false,
                msg_type: MetaCacheEntryType::Object,
                err_no: 0,
                err_msg: String::new(),
            };
            objs.push(info);
        }

        w.write(&objs).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);

        // Skip first 3 entries
        r.skip(3).await.unwrap();

        let remaining = r.read_all().await.unwrap();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].name, "item3");
        assert_eq!(remaining[1].name, "item4");
    }

    #[tokio::test]
    async fn test_metacache_reader_peek_multiple() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let obj = MetaCacheEntry {
            name: "test-item".to_string(),
            metadata: vec![42],
            cached: None,
            reusable: false,
            msg_type: MetaCacheEntryType::Object,
            err_no: 0,
            err_msg: String::new(),
        };

        w.write_obj(&obj).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);

        // First peek should return the object
        let peek1 = r.next().await.unwrap().unwrap();
        assert_eq!(peek1.name, "test-item");

        // Second peek should return None (close entry)
        let peek2 = r.next().await.unwrap();
        assert!(peek2.is_none());
    }

    #[tokio::test]
    async fn test_metacache_entry_type_conversion() {
        assert_eq!(MetaCacheEntryType::Object.to_u8(), 1);
        assert_eq!(MetaCacheEntryType::Error.to_u8(), 2);
        assert_eq!(MetaCacheEntryType::Close.to_u8(), 0);

        assert_eq!(MetaCacheEntryType::from_u8(1), MetaCacheEntryType::Object);
        assert_eq!(MetaCacheEntryType::from_u8(2), MetaCacheEntryType::Error);
        assert_eq!(MetaCacheEntryType::from_u8(0), MetaCacheEntryType::Close);
        assert_eq!(MetaCacheEntryType::from_u8(99), MetaCacheEntryType::Close); // Invalid values default to Close
    }

    #[tokio::test]
    async fn test_metacache_entry_is_dir() {
        let dir_entry = MetaCacheEntry {
            name: "test-dir/".to_string(),
            metadata: Vec::new(),
            ..Default::default()
        };
        assert!(dir_entry.is_dir());

        let file_entry = MetaCacheEntry {
            name: "test-file.txt".to_string(),
            metadata: vec![1, 2, 3],
            ..Default::default()
        };
        assert!(!file_entry.is_dir());

        let dir_no_slash = MetaCacheEntry {
            name: "test-dir".to_string(),
            metadata: Vec::new(),
            ..Default::default()
        };
        assert!(!dir_no_slash.is_dir());
    }

    #[tokio::test]
    async fn test_metacache_entry_is_object() {
        let object_entry = MetaCacheEntry {
            name: "test-object".to_string(),
            metadata: vec![1, 2, 3],
            ..Default::default()
        };
        assert!(object_entry.is_object());

        let dir_entry = MetaCacheEntry {
            name: "test-dir/".to_string(),
            metadata: Vec::new(),
            ..Default::default()
        };
        assert!(!dir_entry.is_object());
    }

    #[tokio::test]
    async fn test_metacache_entry_is_in_dir() {
        // Test file in root directory
        let root_file = MetaCacheEntry {
            name: "file.txt".to_string(),
            ..Default::default()
        };
        assert!(root_file.is_in_dir("", "/"));

        // Test directory in root
        let dir_entry = MetaCacheEntry {
            name: "folder/".to_string(),
            ..Default::default()
        };
        assert!(dir_entry.is_in_dir("", "/"));

        // Test file not in specified directory
        let other_file = MetaCacheEntry {
            name: "other/file.txt".to_string(),
            ..Default::default()
        };
        assert!(!other_file.is_in_dir("folder", "/"));

        // Test direct file in folder with trailing slash
        let direct_file = MetaCacheEntry {
            name: "folder/file.txt".to_string(),
            ..Default::default()
        };
        assert!(direct_file.is_in_dir("folder/", "/"));

        // Test nested file (should not be considered directly in parent folder)
        let nested_file = MetaCacheEntry {
            name: "folder/subfolder/file.txt".to_string(),
            ..Default::default()
        };
        assert!(!nested_file.is_in_dir("folder/", "/")); // Not directly in folder
        assert!(nested_file.is_in_dir("folder/subfolder/", "/")); // Directly in subfolder
    }

    #[tokio::test]
    async fn test_metacache_entry_is_object_dir() {
        let object_dir = MetaCacheEntry {
            name: "object-dir/".to_string(),
            metadata: vec![1, 2, 3],
            ..Default::default()
        };
        assert!(object_dir.is_object_dir());

        let regular_dir = MetaCacheEntry {
            name: "regular-dir/".to_string(),
            metadata: Vec::new(),
            ..Default::default()
        };
        assert!(!regular_dir.is_object_dir());

        let file = MetaCacheEntry {
            name: "file.txt".to_string(),
            metadata: vec![1, 2, 3],
            ..Default::default()
        };
        assert!(!file.is_object_dir());
    }

    #[tokio::test]
    async fn test_metacache_writer_init_multiple_calls() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        // Multiple init calls should not cause issues
        w.init().await.unwrap();
        w.init().await.unwrap();
        w.init().await.unwrap();

        let obj = MetaCacheEntry {
            name: "test".to_string(),
            metadata: vec![1],
            msg_type: MetaCacheEntryType::Object,
            err_no: 0,
            err_msg: String::new(),
            cached: None,
            reusable: false,
        };

        // Use write instead of write_obj to match the working pattern
        w.write(&[obj.clone()]).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let all_objs = r.read_all().await.unwrap();

        assert_eq!(all_objs.len(), 1);
        assert_eq!(all_objs[0].name, "test");
        assert_eq!(all_objs[0].metadata, vec![1]);
    }

    #[tokio::test]
    async fn test_metacache_reader_empty_stream() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        // Just write version and close
        w.init().await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);
        let objs = r.read_all().await.unwrap();

        assert!(objs.is_empty());
    }

    #[tokio::test]
    async fn test_metacache_reader_skip_beyond_available() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let obj = MetaCacheEntry {
            name: "single-item".to_string(),
            metadata: vec![42],
            ..Default::default()
        };

        w.write_obj(&obj).await.unwrap();
        w.close().await.unwrap();

        let data = f.into_inner();
        let nf = Cursor::new(data);

        let mut r = MetacacheReader::new(nf);

        // Skip more than available - should not error
        r.skip(10).await.unwrap();

        let remaining = r.read_all().await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_metacache_writer_empty_name_error() {
        let mut f = Cursor::new(Vec::new());
        let mut w = MetacacheWriter::new(&mut f);

        let obj = MetaCacheEntry {
            name: String::new(), // Empty name should cause error
            metadata: vec![1, 2, 3],
            ..Default::default()
        };

        // write_obj doesn't validate empty names, only write() does
        let result = w.write(&[obj]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no name"));
    }

    #[tokio::test]
    async fn test_metacache_entries_first_found() {
        let entries = MetaCacheEntries(vec![
            None,
            None,
            Some(MetaCacheEntry {
                name: "found".to_string(),
                ..Default::default()
            }),
            Some(MetaCacheEntry {
                name: "also-found".to_string(),
                ..Default::default()
            }),
        ]);

        let (first, total) = entries.first_found();
        assert_eq!(total, 4);
        assert!(first.is_some());
        assert_eq!(first.unwrap().name, "found");
    }

    #[tokio::test]
    async fn test_metacache_entries_first_found_empty() {
        let entries = MetaCacheEntries(vec![None, None, None]);

        let (first, total) = entries.first_found();
        assert_eq!(total, 3);
        assert!(first.is_none());
    }

    #[tokio::test]
    async fn test_metacache_entries_sorted_forward_past() {
        let entries = vec![
            Some(MetaCacheEntry {
                name: "a".to_string(),
                ..Default::default()
            }),
            Some(MetaCacheEntry {
                name: "b".to_string(),
                ..Default::default()
            }),
            Some(MetaCacheEntry {
                name: "c".to_string(),
                ..Default::default()
            }),
            Some(MetaCacheEntry {
                name: "d".to_string(),
                ..Default::default()
            }),
        ];

        let mut sorted = MetaCacheEntriesSorted {
            o: MetaCacheEntries(entries),
            ..Default::default()
        };

        sorted.forward_past(Some("b".to_string()));
        let remaining = sorted.entries();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].name, "c");
        assert_eq!(remaining[1].name, "d");
    }

    #[tokio::test]
    async fn test_metacache_entries_sorted_forward_past_no_marker() {
        let entries = vec![
            Some(MetaCacheEntry {
                name: "a".to_string(),
                ..Default::default()
            }),
            Some(MetaCacheEntry {
                name: "b".to_string(),
                ..Default::default()
            }),
        ];

        let mut sorted = MetaCacheEntriesSorted {
            o: MetaCacheEntries(entries),
            ..Default::default()
        };

        sorted.forward_past(None);
        let remaining = sorted.entries();
        assert_eq!(remaining.len(), 2); // Should remain unchanged
    }
}
