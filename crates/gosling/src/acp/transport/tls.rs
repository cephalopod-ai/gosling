use crate::config::paths::Paths;
use anyhow::{bail, Context, Result};
use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use std::path::Path;
use x509_parser::time::ASN1Time;

#[cfg(feature = "rustls-tls")]
pub type TlsConfig = axum_server::tls_rustls::RustlsConfig;

#[cfg(feature = "native-tls")]
pub type TlsConfig = axum_server::tls_openssl::OpenSSLConfig;

pub struct TlsSetup {
    pub config: TlsConfig,
    pub fingerprint: String,
}

fn generate_self_signed_cert() -> Result<(rcgen::Certificate, KeyPair)> {
    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, "goslingd localhost");
    params.subject_alt_names = vec![
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        SanType::DnsName("localhost".try_into()?),
    ];

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert, key_pair))
}

fn sha256_fingerprint(der: &[u8]) -> String {
    #[cfg(feature = "rustls-tls")]
    {
        let sha256 = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, der);
        sha256
            .as_ref()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    #[cfg(feature = "native-tls")]
    {
        use openssl::hash::MessageDigest;
        let digest =
            openssl::hash::hash(MessageDigest::sha256(), der).expect("SHA-256 hash failed");
        digest
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
}

// An expired certificate can never start working, so refuse it. A certificate that is not yet
// valid is usually clock skew or a pre-issued renewal that becomes usable on its own, so only warn.
fn check_certificate_validity(der: &[u8], cert_path: &Path, now: ASN1Time) -> Result<()> {
    let (_, cert) = x509_parser::parse_x509_certificate(der).with_context(|| {
        format!(
            "invalid X.509 data in TLS certificate {}",
            cert_path.display()
        )
    })?;
    let validity = cert.validity();
    if now > validity.not_after {
        bail!(
            "TLS certificate {} expired on {}; clients will reject it. Renew the certificate.",
            cert_path.display(),
            validity.not_after
        );
    }
    if now < validity.not_before {
        eprintln!(
            "Warning: TLS certificate {} is not valid until {}; clients will reject it until then.",
            cert_path.display(),
            validity.not_before
        );
    }
    Ok(())
}

pub async fn from_pem_files(cert_path: &Path, key_path: &Path) -> Result<TlsSetup> {
    let cert_pem = std::fs::read(cert_path)
        .with_context(|| format!("cannot read TLS certificate {}", cert_path.display()))?;
    let key_pem = std::fs::read(key_path)
        .with_context(|| format!("cannot read TLS private key {}", key_path.display()))?;

    let der = pem::parse(&cert_pem)
        .with_context(|| format!("invalid PEM in TLS certificate {}", cert_path.display()))?
        .into_contents();
    check_certificate_validity(&der, cert_path, ASN1Time::now())?;
    let fingerprint = sha256_fingerprint(&der);
    let load_context = || {
        format!(
            "cannot load TLS certificate {} with private key {}",
            cert_path.display(),
            key_path.display()
        )
    };

    #[cfg(feature = "rustls-tls")]
    let config = {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem.clone())
            .await
            .with_context(load_context)?
    };

    #[cfg(feature = "native-tls")]
    let config = axum_server::tls_openssl::OpenSSLConfig::from_pem(&cert_pem, &key_pem)
        .with_context(load_context)?;

    println!("GOSLINGD_CERT_FINGERPRINT={fingerprint}");
    Ok(TlsSetup {
        config,
        fingerprint,
    })
}

const TLS_PATH_PAIR_REQUIREMENT: &str = "--tls-cert-path/GOSLING_TLS_CERT_PATH and \
     --tls-key-path/GOSLING_TLS_KEY_PATH must both be set, or neither (to use a generated \
     self-signed certificate).";

pub async fn setup_tls(cert_path: Option<&str>, key_path: Option<&str>) -> Result<TlsSetup> {
    match (cert_path, key_path) {
        (Some(cert), Some(key)) => from_pem_files(Path::new(cert), Path::new(key)).await,
        (None, None) => self_signed_config().await,
        (Some(_), None) => bail!(
            "A TLS certificate path was given without a private key path. \
             {TLS_PATH_PAIR_REQUIREMENT}"
        ),
        (None, Some(_)) => bail!(
            "A TLS private key path was given without a certificate path. \
             {TLS_PATH_PAIR_REQUIREMENT}"
        ),
    }
}

fn tls_cache_dir() -> std::path::PathBuf {
    Paths::config_dir().join("tls")
}

fn write_private_key(path: &std::path::Path, contents: &[u8]) {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let result = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path);
        if let Ok(mut file) = result {
            let _ = file.write_all(contents);
        }
    }

    #[cfg(not(unix))]
    {
        let _ = std::fs::write(path, contents);
    }
}

async fn load_cached_tls() -> Option<TlsSetup> {
    let dir = tls_cache_dir();
    let cert_pem = std::fs::read(dir.join("server.pem")).ok()?;
    let key_pem = std::fs::read(dir.join("server.key")).ok()?;

    let der = pem::parse(&cert_pem).ok()?.into_contents();
    let fingerprint = sha256_fingerprint(&der);

    #[cfg(feature = "rustls-tls")]
    let config = axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem)
        .await
        .ok()?;
    #[cfg(feature = "native-tls")]
    let config = axum_server::tls_openssl::OpenSSLConfig::from_pem(&cert_pem, &key_pem).ok()?;

    Some(TlsSetup {
        config,
        fingerprint,
    })
}

fn try_save_tls_to_cache(cert_pem: &str, key_pem: &str) {
    let dir = tls_cache_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("server.pem"), cert_pem);
    write_private_key(&dir.join("server.key"), key_pem.as_bytes());
}

pub async fn self_signed_config() -> Result<TlsSetup> {
    #[cfg(feature = "rustls-tls")]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    if let Some(cached) = load_cached_tls().await {
        println!("GOSLINGD_CERT_FINGERPRINT={}", cached.fingerprint);
        return Ok(cached);
    }

    let (cert, key_pair) = generate_self_signed_cert()?;

    let fingerprint = sha256_fingerprint(cert.der());
    println!("GOSLINGD_CERT_FINGERPRINT={fingerprint}");

    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    try_save_tls_to_cache(&cert_pem, &key_pem);

    #[cfg(feature = "rustls-tls")]
    let config = axum_server::tls_rustls::RustlsConfig::from_pem(
        cert_pem.into_bytes(),
        key_pem.into_bytes(),
    )
    .await?;

    #[cfg(feature = "native-tls")]
    let config =
        axum_server::tls_openssl::OpenSSLConfig::from_pem(cert_pem.as_bytes(), key_pem.as_bytes())?;

    Ok(TlsSetup {
        config,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pair(dir: &Path, cert: &str, key: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let cert_path = dir.join("server.pem");
        let key_path = dir.join("server.key");
        std::fs::write(&cert_path, cert).unwrap();
        std::fs::write(&key_path, key).unwrap();
        (cert_path, key_path)
    }

    fn write_pair_valid_between(
        dir: &Path,
        not_before: (i32, u8, u8),
        not_after: (i32, u8, u8),
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let mut params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        params.not_before = rcgen::date_time_ymd(not_before.0, not_before.1, not_before.2);
        params.not_after = rcgen::date_time_ymd(not_after.0, not_after.1, not_after.2);
        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        write_pair(dir, &cert.pem(), &key_pair.serialize_pem())
    }

    #[tokio::test]
    async fn expired_certificate_is_refused_naming_its_expiry() {
        let temp = tempfile::tempdir().unwrap();
        let (cert_path, key_path) =
            write_pair_valid_between(temp.path(), (2019, 1, 1), (2020, 1, 2));

        let error = format!(
            "{:#}",
            from_pem_files(&cert_path, &key_path).await.err().unwrap()
        );
        assert!(
            error.contains("expired on Jan  2 00:00:00 2020")
                && error.contains(&cert_path.display().to_string()),
            "{error}"
        );
    }

    #[tokio::test]
    async fn not_yet_valid_certificate_still_loads() {
        let temp = tempfile::tempdir().unwrap();
        let (cert_path, key_path) =
            write_pair_valid_between(temp.path(), (2200, 1, 1), (2201, 1, 1));

        from_pem_files(&cert_path, &key_path).await.unwrap();
    }

    #[tokio::test]
    async fn half_a_path_pair_names_the_flags_and_settings() {
        for (cert, key, given) in [
            (
                Some("server.pem"),
                None,
                "certificate path was given without a private key",
            ),
            (
                None,
                Some("server.key"),
                "private key path was given without a certificate",
            ),
        ] {
            let error = format!("{:#}", setup_tls(cert, key).await.err().unwrap());
            assert!(
                error.contains(given)
                    && error.contains("--tls-cert-path/GOSLING_TLS_CERT_PATH")
                    && error.contains("--tls-key-path/GOSLING_TLS_KEY_PATH"),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn pem_file_errors_name_the_file_and_its_role() {
        let temp = tempfile::tempdir().unwrap();
        let (cert, key_pair) = generate_self_signed_cert().unwrap();
        let (cert_path, key_path) = write_pair(temp.path(), &cert.pem(), &key_pair.serialize_pem());

        let missing = temp.path().join("missing.pem");
        let error = format!(
            "{:#}",
            from_pem_files(&missing, &key_path).await.err().unwrap()
        );
        assert!(
            error.contains("TLS certificate") && error.contains("missing.pem"),
            "{error}"
        );

        let error = format!(
            "{:#}",
            from_pem_files(&cert_path, &missing).await.err().unwrap()
        );
        assert!(
            error.contains("TLS private key") && error.contains("missing.pem"),
            "{error}"
        );

        let malformed = temp.path().join("malformed.pem");
        std::fs::write(&malformed, "not a certificate").unwrap();
        let error = format!(
            "{:#}",
            from_pem_files(&malformed, &key_path).await.err().unwrap()
        );
        assert!(
            error.contains("TLS certificate") && error.contains("malformed.pem"),
            "{error}"
        );

        let error = format!(
            "{:#}",
            from_pem_files(&cert_path, &malformed).await.err().unwrap()
        );
        assert!(
            error.contains("private key") && error.contains("malformed.pem"),
            "{error}"
        );

        let setup = from_pem_files(&cert_path, &key_path).await.unwrap();
        assert_eq!(setup.fingerprint, sha256_fingerprint(cert.der()));
    }
}
