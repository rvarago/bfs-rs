//! A backend provided by an s3 bucket.

use super::{Backend, Object};
use async_trait::async_trait;
use aws_config::ConfigLoader;
use aws_sdk_s3::{Client, Endpoint};
use bytes::Bytes;
use eyre::{eyre, Context};
use http::Uri;
use lifterr::IntoOk;
use log::warn;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Options {
    #[serde(rename = "endpoint", with = "opt_uri", default)]
    pub endpoint_uri: Option<Uri>,
}

#[derive(Debug)]
pub(in crate) struct Provider {
    inner: Client,
}

impl Provider {
    pub async fn new(opts: Options) -> Self {
        let config = Self::new_config_with(opts).load().await;
        Self {
            inner: Client::new(&config),
        }
    }

    fn new_config_with(opts: Options) -> ConfigLoader {
        let config = aws_config::from_env();
        if let Some(uri) = opts.endpoint_uri {
            config.endpoint_resolver(Endpoint::immutable(uri))
        } else {
            config
        }
    }
}

#[async_trait]
impl Backend for Provider {
    async fn list_objects(&self, bucket_name: &str) -> eyre::Result<Vec<Object>> {
        self.inner
            .list_objects()
            .bucket(bucket_name)
            .send()
            .await
            .wrap_err_with(|| eyre!("unable to list objects in s3 bucket={}", bucket_name))?
            .contents
            .unwrap_or_default()
            .into_iter()
            .filter_map(try_from_s3_object)
            .collect::<Vec<_>>()
            .into_ok()
    }

    async fn download_object(&self, bucket_name: &str, key: &str) -> eyre::Result<Bytes> {
        self.inner
            .get_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await
            .wrap_err_with(|| {
                eyre!(
                    "unable to download object in s3 bucket={} with key={}",
                    bucket_name,
                    key
                )
            })?
            .body
            .collect()
            .await
            .wrap_err_with(|| {
                eyre!(
                    "unable to read full content of object in s3 bucket={} with key={}",
                    bucket_name,
                    key
                )
            })?
            .into_bytes()
            .into_ok()
    }
}

fn try_from_s3_object(o: aws_sdk_s3::model::Object) -> Option<Object> {
    try_from_s3_object_impl(o)
        .map_err(|e| warn!("unable to extract fields from s3 object, cause={:#}", e))
        .ok()
}

fn try_from_s3_object_impl(o: aws_sdk_s3::model::Object) -> eyre::Result<Object> {
    let name = o.key.ok_or_else(|| eyre!("key not available"))?;
    let size = o.size as u64;
    let last_modified = o
        .last_modified
        .ok_or_else(|| eyre!("last modified not available"))?
        .try_into()
        .wrap_err_with(|| eyre!("last modified cannot be converted into system time"))?;

    Object {
        name,
        size,
        last_modified,
    }
    .into_ok()
}

mod opt_uri {
    use http::uri::Uri;
    use serde::Deserializer;

    pub fn deserialize<'de, D>(de: D) -> Result<Option<Uri>, D::Error>
    where
        D: Deserializer<'de>,
    {
        http_serde::uri::deserialize(de).map(Some)
    }
}
