//! Bounded reads of peer response bodies (magpie discipline: "reads from a
//! peer always carry a limit" — local/调研文档 §12.3). Every HTTP body the
//! store reads outside the rmcp transports goes through `read_body_capped`;
//! the rmcp transport internals carry their own limits (server request body
//! 4 MiB, client SSE events 16 MiB) — the unbounded rmcp gaps (stdio line
//! codec, plain JSON client responses) are upstream and tracked in the
//! gateway hardening backlog.

/// Cap for peer response bodies, magpie's 16 MiB scanner order.
pub(crate) const PEER_BODY_CAP: usize = 16 * 1024 * 1024;

/// Stream a response body in chunks, refusing bodies that exceed `cap`.
/// The cap is enforced on the bytes actually read, so a lying
/// Content-Length cannot bypass it.
pub(crate) async fn read_body_capped(
    mut response: reqwest::Response,
    cap: usize,
    context: &'static str,
) -> Result<Vec<u8>, crate::Error> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        crate::Error::new(
            crate::error::FailureCode::Internal,
            format!("{context} failed: {error}"),
        )
    })? {
        if body.len() + chunk.len() > cap {
            return Err(crate::Error::new(
                crate::error::FailureCode::Internal,
                format!(
                    "{context} failed: response body exceeds the {} MiB read cap",
                    cap / (1024 * 1024)
                ),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Capped read decoded as text (charset nuance degrades to lossy UTF-8 —
/// the cap matters more than the last exotic codepage).
pub(crate) async fn read_text_capped(
    response: reqwest::Response,
    cap: usize,
    context: &'static str,
) -> Result<String, crate::Error> {
    let raw = read_body_capped(response, cap, context).await?;
    Ok(String::from_utf8_lossy(&raw).into_owned())
}
