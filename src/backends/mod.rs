//! Backends for storage services.

pub mod aws;

use async_trait::async_trait;
use std::time::SystemTime;
use tokio::runtime::Runtime;

pub struct BucketConnection {
    service: Box<dyn BucketService>,
    rt: Runtime,
}

impl BucketConnection {
    pub(in crate) fn new<S>(service: S, rt: Runtime) -> Self
    where
        S: 'static + BucketService,
    {
        Self {
            service: Box::new(service),
            rt,
        }
    }

    pub fn list_objects(&self, bucket_name: &str) -> eyre::Result<Vec<Object>> {
        self.rt.block_on(self.service.list_objects(bucket_name))
    }
}

#[async_trait]
pub trait BucketService {
    async fn list_objects(&self, bucket_name: &str) -> eyre::Result<Vec<Object>>;
}

#[derive(Debug)]
pub struct Object {
    pub name: String,
    pub size: u64,
    pub last_modified: SystemTime,
}
