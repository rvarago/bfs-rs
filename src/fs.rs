//! A FUSE backed by a cloud storage.
//!
//! # Workflow
//!
//! For simplicity, buckets are queried upon construction and objects cached for the whole life-time of the filesystem.
//!
//! This implies that operations performed in the buckets *outside the filesystem* are **not** visible to the latter.
//!
//! On the other hand, content stored in objects are downloaded on a per-read basis with **no** caching.
//!
//! Hence, this might be resource-wise prohibitive to some applications.

// TODO: Revisit data definitions such that we can make queries more efficient (e.g. avoid linear scan upon reading files).

use crate::backends::{BlockingConnection, Object};
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
};
use libc::{EIO, ENOENT};
use lifterr::IntoOk;
use log::{debug, warn};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct BucketFilesystem {
    bucket_name: String,
    attrs: Attrs,
    inodes: Inodes,

    conn: BlockingConnection,
}

type Attrs = HashMap<u64, FileAttr>;
type Inodes = HashMap<OsString, u64>;

const ROOT_INO: u64 = 1;
const ROOT_PATH: &str = "/";

const TTL: Duration = Duration::from_secs(1);

impl BucketFilesystem {
    pub fn new(bucket_name: String, conn: BlockingConnection) -> eyre::Result<Self> {
        let objects = conn.list_objects(&bucket_name)?;

        let (attrs, inodes) = Self::new_fs_from(objects);

        Self {
            bucket_name,
            attrs,
            inodes,
            conn,
        }
        .into_ok()
    }

    fn new_fs_from(objects: Vec<Object>) -> (Attrs, Inodes) {
        let (mut attrs, mut inodes) = Self::new_childs_from(objects);

        let (root_size, root_mtime) =
            attrs.values().fold((0, UNIX_EPOCH), |(size, mtime), attr| {
                (size + attr.size, mtime.max(attr.mtime))
            });

        attrs.insert(
            ROOT_INO,
            Self::new_root_attr(ROOT_INO, root_size, root_mtime),
        );

        inodes.insert(ROOT_PATH.into(), ROOT_INO);

        (attrs, inodes)
    }

    fn new_childs_from(objects: Vec<Object>) -> (Attrs, Inodes) {
        objects
            .into_iter()
            .enumerate()
            .map(|(i, object)| {
                let ino = i as u64 + 2;
                let attr = Self::new_child_attr(ino, object.size, object.last_modified);
                ((ino, attr), (object.name.into(), ino))
            })
            .unzip()
    }

    fn new_root_attr(ino: u64, size: u64, mtime: SystemTime) -> FileAttr {
        Self::new_attr(ino, FileType::Directory, size, mtime)
    }

    fn new_child_attr(ino: u64, size: u64, mtime: SystemTime) -> FileAttr {
        Self::new_attr(ino, FileType::RegularFile, size, mtime)
    }

    fn new_attr(ino: u64, kind: FileType, size: u64, mtime: SystemTime) -> FileAttr {
        const BLOCK_SIZE: u32 = 512;

        FileAttr {
            ino,
            size,
            blocks: (size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64,
            atime: UNIX_EPOCH,
            mtime,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind,
            perm: 0o444, // -r--r--r--
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }
}

impl Filesystem for BucketFilesystem {
    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        debug!("getattr(ino={})", ino);

        match self.attrs.get(&ino) {
            Some(attr) => reply.attr(&TTL, attr),
            None => {
                warn!("attempted to get attrs of non-existent file, ino={}", ino);
                reply.error(ENOENT)
            }
        }
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        debug!("lookup(parent={}, name={})", parent, name.to_string_lossy());

        match self.inodes.get(name).and_then(|ino| {
            debug!("looked up ino={} by name={}", ino, name.to_string_lossy());
            self.attrs.get(ino)
        }) {
            Some(attr) => reply.entry(&TTL, attr, 0),
            None => reply.error(ENOENT),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        debug!("readdir(ino={}, fh={}, offset={})", ino, fh, offset);

        if ino == ROOT_INO {
            if offset == 0 {
                for (path, ino) in &self.inodes {
                    let offset = *ino as i64;
                    let kind = self.attrs.get(ino).map(|o| o.kind).unwrap(); // The relationship between inodes and attrs has been established upon construction.
                    if reply.add(*ino, offset, kind, path) {
                        break;
                    }
                }
            }
            reply.ok();
        } else {
            warn!("attempted to read non-root dir, ino={}", ino);
            reply.error(ENOENT);
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        debug!(
            "read(ino={}, fh={}, offset={}, size={})",
            ino, fh, offset, size
        );

        match self
            .inodes
            .iter()
            .find_map(|(path, i)| (*i == ino).then(|| path))
        {
            Some(path) => {
                let path = path.to_string_lossy();
                match self.conn.download_object(&self.bucket_name, path.as_ref()) {
                    Ok(content) => reply.data(content.as_ref()),
                    Err(e) => {
                        warn!("unable download object, cause={:#}", e);
                        reply.error(EIO)
                    }
                }
            }
            None => {
                warn!("attempted to read from non-existent file, ino={}", ino);
                reply.error(ENOENT)
            }
        }
    }
}
