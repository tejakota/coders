use crate::ProviderConfig;
use anyhow::{Context, Result};
use reqwest::Client;

/// Builds the shared HTTP client, trusting an extra CA cert (e.g. a
/// corporate VPN/proxy inspection root) on top of the OS's native trust
/// store when `extra_ca_cert` points at a PEM/CRT file.
pub(crate) fn build_client(config: &ProviderConfig) -> Result<Client> {
    let mut builder = Client::builder();
    if let Some(path) = &config.extra_ca_cert {
        let pem = std::fs::read(path).with_context(|| format!("reading extra_ca_cert '{path}'"))?;
        let cert = reqwest::Certificate::from_pem(&pem).with_context(|| format!("parsing extra_ca_cert '{path}' as PEM"))?;
        builder = builder.add_root_certificate(cert);
    }
    builder.build().context("building HTTP client")
}
