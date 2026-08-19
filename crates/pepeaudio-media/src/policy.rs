use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use async_trait::async_trait;
use tokio::time::{Instant, timeout_at};
use url::{Host, Url};

use crate::UrlPolicyError;

const MAX_DNS_ADDRESSES: usize = 32;

/// Asynchronous name-resolution boundary used before every request hop.
#[async_trait]
pub trait DnsResolver: Send + Sync {
    /// Resolves a normalized domain name to every candidate address.
    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>, UrlPolicyError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TokioDnsResolver;

#[async_trait]
impl DnsResolver for TokioDnsResolver {
    async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>, UrlPolicyError> {
        tokio::net::lookup_host((domain, 0))
            .await
            .map(|addresses| {
                addresses
                    .take(MAX_DNS_ADDRESSES + 1)
                    .map(|address| address.ip())
                    .collect()
            })
            .map_err(|_| UrlPolicyError::Dns)
    }
}

/// Parser and network policy applied independently to every redirect hop.
#[derive(Clone, Copy, Debug)]
pub struct UrlGuard {
    max_url_bytes: usize,
    dns_timeout: Duration,
}

impl UrlGuard {
    #[must_use]
    pub const fn new(max_url_bytes: usize, dns_timeout: Duration) -> Self {
        Self {
            max_url_bytes,
            dns_timeout,
        }
    }

    /// Validates syntax, resolves domains, rejects every unsafe answer, and
    /// returns addresses that an HTTP adapter can pin.
    ///
    /// # Errors
    ///
    /// Returns [`UrlPolicyError`] for malformed or unsafe URLs and failed,
    /// timed-out, empty, excessive, or unsafe DNS answers.
    pub async fn approve(
        &self,
        raw_url: &str,
        resolver: &dyn DnsResolver,
    ) -> Result<ApprovedUrl, UrlPolicyError> {
        let url = self.parse(raw_url)?;
        let port = url
            .port_or_known_default()
            .ok_or(UrlPolicyError::MissingAuthority)?;

        let (domain, addresses) = match url.host() {
            Some(Host::Ipv4(address)) => (None, vec![IpAddr::V4(address)]),
            Some(Host::Ipv6(address)) => (None, vec![IpAddr::V6(address)]),
            Some(Host::Domain(domain)) if !domain.is_empty() => {
                let dns_deadline = Instant::now()
                    .checked_add(self.dns_timeout)
                    .ok_or(UrlPolicyError::DnsTimeout(self.dns_timeout))?;
                let dns_answers = timeout_at(dns_deadline, resolver.resolve(domain))
                    .await
                    .map_err(|_| UrlPolicyError::DnsTimeout(self.dns_timeout))??;
                (Some(domain.to_owned()), dns_answers)
            }
            _ => return Err(UrlPolicyError::MissingAuthority),
        };

        let mut unique = HashSet::new();
        let addresses: Vec<_> = addresses
            .into_iter()
            .filter(|address| unique.insert(*address))
            .collect();
        if addresses.is_empty() {
            return Err(UrlPolicyError::EmptyDnsAnswer);
        }
        if addresses.len() > MAX_DNS_ADDRESSES {
            return Err(UrlPolicyError::TooManyDnsAnswers);
        }
        if addresses.iter().any(|address| is_forbidden_ip(*address)) {
            return Err(UrlPolicyError::ForbiddenAddress);
        }

        Ok(ApprovedUrl {
            url,
            domain,
            addresses: addresses
                .into_iter()
                .map(|address| SocketAddr::new(address, port))
                .collect(),
        })
    }

    pub(crate) fn join(&self, base: &Url, location: &str) -> Result<String, UrlPolicyError> {
        if location.len() > self.max_url_bytes {
            return Err(UrlPolicyError::TooLong {
                max_bytes: self.max_url_bytes,
            });
        }
        if raw_has_userinfo_marker(location) {
            return Err(UrlPolicyError::UserInfo);
        }
        let joined = base.join(location).map_err(|_| UrlPolicyError::Malformed)?;
        self.validate_parsed(&joined)?;
        if base.scheme() == "https" && joined.scheme() == "http" {
            return Err(UrlPolicyError::InsecureRedirect);
        }
        Ok(joined.into())
    }

    fn parse(&self, raw_url: &str) -> Result<Url, UrlPolicyError> {
        if raw_url.len() > self.max_url_bytes {
            return Err(UrlPolicyError::TooLong {
                max_bytes: self.max_url_bytes,
            });
        }
        if raw_has_userinfo_marker(raw_url) {
            return Err(UrlPolicyError::UserInfo);
        }
        let url = Url::parse(raw_url).map_err(|_| UrlPolicyError::Malformed)?;
        self.validate_parsed(&url)?;
        Ok(url)
    }

    fn validate_parsed(&self, url: &Url) -> Result<(), UrlPolicyError> {
        if url.as_str().len() > self.max_url_bytes {
            return Err(UrlPolicyError::TooLong {
                max_bytes: self.max_url_bytes,
            });
        }
        if !matches!(url.scheme(), "http" | "https") {
            return Err(UrlPolicyError::UnsupportedScheme);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(UrlPolicyError::UserInfo);
        }
        if url.fragment().is_some() {
            return Err(UrlPolicyError::Fragment);
        }
        if url.host().is_none() || url.port_or_known_default().is_none() {
            return Err(UrlPolicyError::MissingAuthority);
        }
        Ok(())
    }
}

/// A URL plus the public addresses inspected for its current hop.
#[derive(Clone, Debug)]
pub struct ApprovedUrl {
    url: Url,
    domain: Option<String>,
    addresses: Vec<SocketAddr>,
}

impl ApprovedUrl {
    #[must_use]
    pub const fn url(&self) -> &Url {
        &self.url
    }

    /// Domain to override in a resolver-aware HTTP client.
    #[must_use]
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Already-inspected endpoint candidates, including the effective port.
    #[must_use]
    pub fn socket_addrs(&self) -> &[SocketAddr] {
        &self.addresses
    }

    #[cfg(test)]
    pub(crate) fn test_only(url: Url, domain: String, addresses: Vec<SocketAddr>) -> Self {
        Self {
            url,
            domain: Some(domain),
            addresses,
        }
    }
}

#[must_use]
pub fn is_forbidden_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => forbidden_v4(address),
        IpAddr::V6(address) => forbidden_v6(address),
    }
}

fn forbidden_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_documentation()
        || first == 0
        || (first == 100 && (64..=127).contains(&second))
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 192 && second == 0)
        || (first == 192 && second == 88 && address.octets()[2] == 99)
        || first >= 240
}

fn forbidden_v6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    if let Some(mapped) = address.to_ipv4_mapped() {
        return forbidden_v4(mapped);
    }

    let ipv4_compatible = segments[..6].iter().all(|segment| *segment == 0);
    let nat64_well_known = segments[0] == 0x0064
        && segments[1] == 0xff9b
        && segments[2..6].iter().all(|segment| *segment == 0);
    let [embedded_a, embedded_b] = segments[6].to_be_bytes();
    let [embedded_c, embedded_d] = segments[7].to_be_bytes();
    let embedded_v4 = Ipv4Addr::new(embedded_a, embedded_b, embedded_c, embedded_d);
    let [six_to_four_a, six_to_four_b] = segments[1].to_be_bytes();
    let [six_to_four_c, six_to_four_d] = segments[2].to_be_bytes();
    let six_to_four_v4 = Ipv4Addr::new(six_to_four_a, six_to_four_b, six_to_four_c, six_to_four_d);

    address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || address.is_unique_local()
        || address.is_unicast_link_local()
        || (segments[0] & 0xffc0) == 0xfec0
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x0100 && segments[1..4].iter().all(|segment| *segment == 0))
        || ipv4_compatible
        || (nat64_well_known && forbidden_v4(embedded_v4))
        || (segments[0] == 0x0064 && segments[1] == 0xff9b && segments[2] == 1)
        || (segments[0] == 0x2002 && forbidden_v4(six_to_four_v4))
        || (segments[0] == 0x2001 && segments[1] == 0)
        || (segments[0] == 0x2001 && segments[1] == 2)
        || (segments[0] == 0x2001 && (segments[1] & 0xfff0) == 0x0020)
}

fn raw_has_userinfo_marker(raw: &str) -> bool {
    let authority = if let Some((_, remainder)) = raw.split_once(':') {
        remainder.trim_start_matches(['/', '\\'])
    } else if raw.starts_with("//") || raw.starts_with("\\\\") {
        raw.trim_start_matches(['/', '\\'])
    } else {
        return false;
    };
    authority
        .split(['/', '\\', '?', '#'])
        .next()
        .is_some_and(|candidate| candidate.contains('@'))
}
