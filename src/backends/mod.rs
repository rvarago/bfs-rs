//! Backends for storage services.

pub mod aws;

use async_trait::async_trait;
use bytes::Bytes;
use std::time::SystemTime;
use tokio::runtime::Runtime;

pub struct BlockingConnection {
    service: Box<dyn Backend>,
    rt: Runtime,
}

impl BlockingConnection {
    pub(in crate) fn new<S>(service: S, rt: Runtime) -> Self
    where
        S: 'static + Backend,
    {
        Self {
            service: Box::new(service),
            rt,
        }
    }

    pub fn list_objects(&self, bucket_name: &str) -> eyre::Result<Vec<Object>> {
        self.rt.block_on(self.service.list_objects(bucket_name))
    }

    pub fn download_object(&self, bucket_name: &str, key: &str) -> eyre::Result<Bytes> {
        self.rt
            .block_on(self.service.download_object(bucket_name, key))
    }
}

/// An interface to a cloud-storage.
#[async_trait]
pub trait Backend {
    async fn list_objects(&self, bucket_name: &str) -> eyre::Result<Vec<Object>>;

    async fn download_object(&self, bucket_name: &str, key: &str) -> eyre::Result<Bytes>;
}

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub size: u64,
    pub last_modified: SystemTime,
}
