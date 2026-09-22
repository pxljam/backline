//! S3-compatible object storage (MinIO locally, OVH Object Storage in prod).
//!
//! Raw footage is hosted (§10.1): the render CLI downloads media through
//! signed, time-limited URLs.

use crate::config::S3Config;
use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::{Bucket, Region};

#[derive(Clone)]
pub struct Storage {
    /// Internal client (Docker network) — server-side reads and writes.
    inner: Box<Bucket>,
    /// Client on the public endpoint — only used to sign URLs usable from a
    /// browser or a render machine.
    public: Box<Bucket>,
    pub bucket_name: String,
    creds: Credentials,
}

impl Storage {
    pub fn new(cfg: &S3Config) -> Result<Self> {
        let creds = Credentials::new(
            Some(&cfg.access_key),
            Some(&cfg.secret_key),
            None,
            None,
            None,
        )
        .context("identifiants S3 invalides")?;

        let mk = |endpoint: &str| -> Result<Box<Bucket>> {
            let region = Region::Custom {
                region: cfg.region.clone(),
                endpoint: endpoint.trim_end_matches('/').to_string(),
            };
            Ok(Bucket::new(&cfg.bucket, region, creds.clone())?.with_path_style())
        };

        Ok(Self {
            inner: mk(&cfg.endpoint)?,
            public: mk(&cfg.public_endpoint)?,
            bucket_name: cfg.bucket.clone(),
            creds,
        })
    }

    /// Creates the bucket if missing. Idempotent — called at startup so that
    /// `docker compose up` on a clean machine is enough.
    pub async fn ensure_bucket(&self) -> Result<()> {
        if self.inner.exists().await.unwrap_or(false) {
            return Ok(());
        }
        let cfg = s3::BucketConfiguration::default();
        match Bucket::create_with_path_style(
            &self.bucket_name,
            self.inner.region(),
            self.creds.clone(),
            cfg,
        )
        .await
        {
            Ok(_) => Ok(()),
            // A race between instances at startup: of no consequence.
            Err(e) if format!("{e:?}").contains("BucketAlreadyOwnedByYou") => Ok(()),
            Err(e) => Err(e).context("creation du bucket"),
        }
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> Result<()> {
        self.inner
            .put_object_with_content_type(key, &bytes, content_type)
            .await
            .with_context(|| format!("televersement de {key}"))?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .inner
            .get_object(key)
            .await
            .with_context(|| format!("lecture de {key}"))?;
        Ok(resp.bytes().to_vec())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.inner.delete_object(key).await?;
        Ok(())
    }

    /// Signed URL, valid for `seconds`. This is what the render CLI receives.
    pub async fn signed_url(&self, key: &str, seconds: u32) -> Result<String> {
        Ok(self.public.presign_get(key, seconds, None).await?)
    }

    pub async fn signed_put_url(&self, key: &str, seconds: u32) -> Result<String> {
        Ok(self.public.presign_put(key, seconds, None, None).await?)
    }
}
