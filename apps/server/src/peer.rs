//! Client address for rate limits.
//!
//! `OWLEE_PROXY_HOPS` is how many reverse proxies append to `X-Forwarded-For`.
//! The trusted address is that many entries from the right. `0` uses the TCP
//! peer and ignores the header, so a proxy in front collapses every visitor
//! into the proxy's address.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;
use owlee_insights::clip;

use crate::state::AppState;

pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let peer = peer_addr(&parts.extensions);
        Ok(Self(client_ip(
            &parts.headers,
            state.config.proxy_hops,
            peer,
        )))
    }
}

pub fn client_ip(headers: &HeaderMap, proxy_hops: usize, peer: Option<IpAddr>) -> String {
    let forwarded = (proxy_hops > 0)
        .then(|| {
            headers
                .get("x-forwarded-for")?
                .to_str()
                .ok()?
                .split(',')
                .rev()
                .nth(proxy_hops - 1)
        })
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| clip(value, 64));
    forwarded
        .or_else(|| peer.map(|ip| ip.to_string()))
        .unwrap_or_else(|| "0.0.0.0".to_owned())
}

pub fn peer_addr(extensions: &axum::http::Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", value.parse().unwrap());
        headers
    }

    #[test]
    fn one_proxy_uses_the_appended_client() {
        let peer = "10.0.0.8".parse().unwrap();
        assert_eq!(
            client_ip(&headers("203.0.113.9"), 1, Some(peer)),
            "203.0.113.9"
        );
        assert_eq!(
            client_ip(&headers("198.51.100.4, 203.0.113.9"), 1, Some(peer)),
            "203.0.113.9"
        );
    }

    #[test]
    fn zero_hops_ignores_the_header() {
        let peer = "10.0.0.8".parse().unwrap();
        assert_eq!(
            client_ip(&headers("203.0.113.9"), 0, Some(peer)),
            "10.0.0.8"
        );
    }

    #[test]
    fn two_hops_skips_cloudflare_and_traefik() {
        let peer = "10.0.0.8".parse().unwrap();
        assert_eq!(
            client_ip(
                &headers("203.0.113.9, 172.64.0.1, 10.0.0.8"),
                2,
                Some(peer)
            ),
            "172.64.0.1"
        );
    }
}
