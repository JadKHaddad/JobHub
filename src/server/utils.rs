#[derive(Debug, thiserror::Error)]
pub enum GoogleConvertLinkError {
    #[error("Invalid scheme: {0}")]
    InvalidScheme(String),
    #[error("Invalid host: {0}")]
    InvalidHost(String),
    #[error("No host")]
    NoHost,
    #[error("No id in path")]
    NoIdInPath,
    #[error("No segments")]
    NoSegments,
}

pub fn convert_google_share_or_view_url_to_download_url(
    share_url: url::Url,
) -> Result<url::Url, GoogleConvertLinkError> {
    let schme = share_url.scheme();
    if schme != "https" {
        return Err(GoogleConvertLinkError::InvalidScheme(schme.to_string()));
    }

    let host = share_url.host_str().ok_or(GoogleConvertLinkError::NoHost)?;
    if host != "drive.google.com" {
        return Err(GoogleConvertLinkError::InvalidHost(host.to_string()));
    }

    // get the third path segment
    let id = share_url
        .path_segments()
        .ok_or(GoogleConvertLinkError::NoSegments)?
        .nth(2)
        .ok_or(GoogleConvertLinkError::NoIdInPath)?;

    let mut download_url =
        url::Url::parse("https://drive.google.com/uc?export=download").expect("hardcoded url");
    download_url.query_pairs_mut().append_pair("id", id);

    Ok(download_url)
}
