//! A FUSE backed by a cloud storage.
//!
//! # Workflow
//!
//! For simplicity, buckets are queried upon construction and objects cached for the whole life-time of the filesystem.
//!
//! This implies that operations performed in the buckets *outside the filesystem* are **not** visible to the latter.

use crate::backends::{BlockingConnection, Object};
use fuser::{FileAttr, FileType, Filesystem, ReplyAttr, ReplyDirectory, ReplyEntry, Request};
use libc::ENOENT;
use lifterr::IntoOk;
use log::{debug, warn};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub struct BucketFilesystem {
    attrs: Attrs,
    inodes: Inodes,
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

        Self { attrs, inodes }.into_ok()
    }

    fn new_fs_from(objects: Vec<Object>) -> (Attrs, Inodes) {
        let mut attrs = HashMap::default();
        let mut inodes = HashMap::default();

        attrs.insert(ROOT_INO, Self::new_root_attr());
        inodes.insert(ROOT_PATH.into(), ROOT_INO);

        objects.into_iter().enumerate().for_each(|(ino, object)| {
            let attr = Self::new_entry_attr(ino as u64 + 2, &object);
            attrs.insert(attr.ino, attr);
            inodes.insert(object.name.into(), attr.ino);
        });

        (attrs, inodes)
    }

    fn new_root_attr() -> FileAttr {
        Self::new_attr(ROOT_INO, FileType::Directory, 0, UNIX_EPOCH)
    }

    fn new_entry_attr(ino: u64, object: &Object) -> FileAttr {
        Self::new_attr(
            ino,
            FileType::RegularFile,
            object.size,
            object.last_modified,
        )
    }

    fn new_attr(ino: u64, kind: FileType, size: u64, mtime: SystemTime) -> FileAttr {
        FileAttr {
            ino,
            size,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind,
            perm: 0o444, // -r--r--r--
            nlink: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 0,
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
}
