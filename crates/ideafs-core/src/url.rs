use url::Url;

pub fn validate_http_url(url: &str) -> Result<String, String> {
    let parsed = Url::parse(url).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Only HTTP and HTTPS URLs can be opened".to_owned());
    }
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_opener_accepts_http_and_rejects_other_schemes() {
        assert_eq!(
            validate_http_url("https://example.com/path").as_deref(),
            Ok("https://example.com/path")
        );
        assert!(validate_http_url("file:///C:/secret.txt").is_err());
        assert!(validate_http_url("javascript:alert(1)").is_err());
    }
}