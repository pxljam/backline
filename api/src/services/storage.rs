//! Stockage objet S3-compatible (MinIO en local, Object Storage OVH en prod).
//!
//! Les rushes sont heberges (§10.1) : la CLI de rendu telecharge les medias
//! par URL signees, a duree limitee.

use crate::config::S3Config;
use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::{Bucket, Region};

#[derive(Clone)]
pub struct Storage {
    /// Client interne (reseau Docker) — lectures et ecritures serveur.
    inner: Box<Bucket>,
    /// Client sur l'endpoint public — sert uniquement a signer des URL
    /// utilisables depuis un navigateur ou une machine de rendu.
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

    /// Cree le bucket s'il manque. Idempotent — appele au demarrage pour que
    /// `docker compose up` sur une machine vierge suffise.
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
            // Course entre plusieurs instances au demarrage : sans consequence.
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

    /// URL signee, valable `seconds`. C'est ce que recoit la CLI de rendu.
    pub async fn signed_url(&self, key: &str, seconds: u32) -> Result<String> {
        Ok(self.public.presign_get(key, seconds, None).await?)
    }

    pub async fn signed_put_url(&self, key: &str, seconds: u32) -> Result<String> {
        Ok(self.public.presign_put(key, seconds, None, None).await?)
    }
}
