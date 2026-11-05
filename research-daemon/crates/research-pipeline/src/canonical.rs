use url::Url;

/// Normalize a URL for deduplication.
///
/// Fragments, tracking parameters, default ports, and trailing slashes are
/// removed. Non-HTTP(S) URLs and malformed input are rejected.
pub fn canonicalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    let host = url.host_str()?.to_ascii_lowercase();
    url.set_host(Some(&host)).ok()?;
    url.set_fragment(None);

    let default_port = matches!(
        (url.scheme(), url.port()),
        ("http", Some(80)) | ("https", Some(443))
    );
    if default_port {
        url.set_port(None).ok()?;
    }

    let retained: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking_param(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if retained.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(retained);
    }

    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(if path.is_empty() { "/" } else { &path });

    Some(url.to_string())
}

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_")
        || matches!(
            key.as_str(),
            "fbclid" | "gclid" | "mc_cid" | "mc_eid" | "ref" | "ref_src" | "source"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_hosts_fragments_and_trailing_slashes() {
        assert_eq!(
            canonicalize_url("HTTPS://Example.COM/Path/?utm_source=news#section").unwrap(),
            "https://example.com/Path"
        );
    }

    #[test]
    fn keeps_meaningful_query_parameters() {
        assert_eq!(
            canonicalize_url("https://example.com/doc?page=2&utm_medium=email").unwrap(),
            "https://example.com/doc?page=2"
        );
    }

    #[test]
    fn removes_default_ports_and_rejects_non_web_urls() {
        assert_eq!(
            canonicalize_url("https://example.com:443/a").unwrap(),
            "https://example.com/a"
        );
        assert!(canonicalize_url("mailto:someone@example.com").is_none());
        assert!(canonicalize_url("not a url").is_none());
    }
}
