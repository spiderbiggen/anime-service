use std::any;

/// Errors which may occur when creating form data.
#[derive(Debug, thiserror::Error)]
pub enum BodyError {
    /// Body data could not be serialized from form parameters.
    #[error("failed to URL encode form parameters: {}", source)]
    UrlEncoded {
        /// The source of the error.
        #[from]
        source: serde_urlencoded::ser::Error,
    },
    /// Body data could not be serialized from form parameters.
    #[error("failed to json encode body: {}", source)]
    JsonEncoded {
        /// The source of the error.
        #[from]
        source: serde_json::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The URL failed to parse.
    #[error("failed to parse url: {}", source)]
    UrlParse {
        /// The source of the error.
        #[from]
        source: url::ParseError,
    },
    #[error("failed to parse uri: {}", source)]
    UriParse {
        /// The source of the error.
        #[from]
        source: http::uri::InvalidUri,
    },
    /// Body data could not be created.
    #[error("failed to create form data: {}", source)]
    Body {
        /// The source of the error.
        #[from]
        source: BodyError,
    },
    #[error("failed to complete http request: {}", source)]
    Http {
        /// The source of the error.
        #[from]
        source: http::Error,
    },
    /// JSON deserialization failed.
    #[error("could not parse JSON response: {}", source)]
    Json {
        /// The source of the error.
        #[from]
        source: serde_json::Error,
    },
    /// Service returned an error without JSON information.
    #[error("internal server error {}", status)]
    Service {
        /// The status code for the return.
        status: http::StatusCode,
        /// The error data from the service.
        data: Vec<u8>,
    },
    /// Failed to parse an expected data type from JSON.
    #[error("could not parse {} data from JSON: {}", typename, source)]
    DataType {
        /// The source of the error.
        source: serde_json::Error,
        /// The name of the type that could not be deserialized.
        typename: &'static str,
    },
    #[cfg(feature = "hyper")]
    #[error("hyper client failed: {}", source)]
    Hyper {
        /// The error data from the service.
        #[from]
        source: hyper::Error,
    },
}

impl ApiError {
    pub(crate) fn server_error(status: http::StatusCode, body: &bytes::Bytes) -> Self {
        Self::Service {
            status,
            data: body.into_iter().copied().collect(),
        }
    }

    pub(crate) fn data_type<T>(source: serde_json::Error) -> Self {
        ApiError::DataType {
            source,
            typename: any::type_name::<T>(),
        }
    }
}
