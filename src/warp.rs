//! Adapter for the `warp` HTTP server framework.
//!
//! The filters in this module will always succeed and never
//! return an error. For example, if a file is not found, the
//! filter will return a 404 reply, and not an internal
//! rejection.
//!
use std::convert::Infallible;
use std::path::Path;

use crate::{fakels::FakeLs, localfs::LocalFs, time::UtcOffset, DavHandler};
use warp::{filters::BoxedFilter, Filter, Reply};

/// Reply-filter that runs a DavHandler.
///
/// Just pass in a pre-configured DavHandler. If a prefix was not
/// configured, it will be the request path up to this point.
pub fn dav_handler(handler: DavHandler) -> BoxedFilter<(impl Reply,)> {
    use warp::http::header::HeaderMap;
    use warp::http::Method;
    use warp::path::{FullPath, Tail};

    warp::method()
        .and(warp::path::full())
        .and(warp::path::tail())
        .and(warp::header::headers_cloned())
        .and(warp::body::stream())
        .and_then(
            move |method: Method, path_full: FullPath, path_tail: Tail, headers: HeaderMap, body| {
                let handler = handler.clone();

                async move {
                    // rebuild an http::Request struct.
                    let path_str = path_full.as_str();
                    let uri = path_str.parse::<http::uri::Uri>().unwrap();
                    let mut builder = http::Request::builder().method(method.as_str()).uri(uri);
                    for (k, v) in headers.iter() {
                        builder = builder.header(k.as_str(), v.as_ref());
                    }
                    let request = builder.body(body).unwrap();

                    let response = if handler.config.prefix.is_some() {
                        // Run a handler with the configured path prefix.
                        handler.handle_stream(request).await
                    } else {
                        // Run a handler with the current path prefix.
                        let path_len = path_str.len();
                        let tail_len = path_tail.as_str().len();
                        let prefix = path_str[..path_len - tail_len].to_string();
                        let config = DavHandler::builder().strip_prefix(prefix);
                        handler.handle_stream_with(config, request).await
                    };

                    // Need to remap the http_body::Body to a hyper::Body.
                    let (parts, body) = response.into_parts();

                    let http_version = match parts.version {
                        http::Version::HTTP_09 => Some(warp::http::Version::HTTP_09),
                        http::Version::HTTP_10 => Some(warp::http::Version::HTTP_10),
                        http::Version::HTTP_11 => Some(warp::http::Version::HTTP_11),
                        http::Version::HTTP_2 => Some(warp::http::Version::HTTP_2),
                        http::Version::HTTP_3 => Some(warp::http::Version::HTTP_3),
                        _ => None,
                    };
                    let response = match http_version {
                        Some(http_version) => warp::http::response::Response::builder()
                            .status(parts.status.as_u16())
                            .version(http_version)
                            .body(warp::hyper::Body::wrap_stream(body))
                            .unwrap(),
                        None => warp::http::response::Response::builder()
                            .status(warp::http::StatusCode::INTERNAL_SERVER_ERROR)
                            .body(warp::hyper::Body::empty())
                            .unwrap(),
                    };
                    Ok::<_, Infallible>(response)
                }
            },
        )
        .boxed()
}

/// Creates a Filter that serves files and directories at the
/// base path joined with the remainder of the request path,
/// like `warp::filters::fs::dir`.
///
/// The behaviour for serving a directory depends on the flags:
///
/// - `index_html`: if an `index.html` file is found, serve it.
/// - `auto_index`: create a directory listing.
/// - no flags set: 404.
pub fn dav_dir(
    base: impl AsRef<Path>,
    index_html: bool,
    auto_index: bool,
    utc_offset: Option<UtcOffset>,
) -> BoxedFilter<(impl Reply,)> {
    let mut builder = DavHandler::builder()
        .filesystem(LocalFs::new(base, false, false, false))
        .locksystem(FakeLs::new())
        .autoindex(auto_index, utc_offset);
    if index_html {
        builder = builder.indexfile("index.html".to_string())
    }
    let handler = builder.build_handler();
    dav_handler(handler)
}

/// Creates a Filter that serves a single file, ignoring the request path,
/// like `warp::filters::fs::file`.
pub fn dav_file(file: impl AsRef<Path>) -> BoxedFilter<(impl Reply,)> {
    let handler = DavHandler::builder()
        .filesystem(LocalFs::new_file(file, false))
        .locksystem(FakeLs::new())
        .build_handler();
    dav_handler(handler)
}
