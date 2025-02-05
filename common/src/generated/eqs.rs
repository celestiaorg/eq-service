// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetKeccakInclusionRequest {
    /// Data Avaliblity block height
    #[prost(uint64, tag = "1")]
    pub height: u64,
    /// 32 byte DA namespace
    #[prost(bytes = "vec", tag = "2")]
    pub namespace: ::prost::alloc::vec::Vec<u8>,
    /// 32 byte DA blob commitment
    #[prost(bytes = "vec", tag = "3")]
    pub commitment: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetKeccakInclusionResponse {
    #[prost(enumeration = "get_keccak_inclusion_response::Status", tag = "1")]
    pub status: i32,
    #[prost(oneof = "get_keccak_inclusion_response::ResponseValue", tags = "2, 3, 4, 5")]
    pub response_value: ::core::option::Option<
        get_keccak_inclusion_response::ResponseValue,
    >,
}
/// Nested message and enum types in `GetKeccakInclusionResponse`.
pub mod get_keccak_inclusion_response {
    #[derive(
        Clone,
        Copy,
        Debug,
        PartialEq,
        Eq,
        Hash,
        PartialOrd,
        Ord,
        ::prost::Enumeration
    )]
    #[repr(i32)]
    pub enum Status {
        /// Data Avaliblity (DA) inclusion proof being collected
        DaPending = 0,
        /// DA inclusion proof collected
        DaAvalible = 1,
        /// Zero Knowledge Proof (ZKP) of DA inclusion requested, generating
        ZkpPending = 2,
        /// ZKP of DA inclusion proof finished
        ZkpFinished = 3,
        /// If this is returned, the service then attempts to retry
        RetryableFailure = 4,
        /// No way to complete request
        PermanentFailure = 5,
    }
    impl Status {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Status::DaPending => "DA_PENDING",
                Status::DaAvalible => "DA_AVALIBLE",
                Status::ZkpPending => "ZKP_PENDING",
                Status::ZkpFinished => "ZKP_FINISHED",
                Status::RetryableFailure => "RETRYABLE_FAILURE",
                Status::PermanentFailure => "PERMANENT_FAILURE",
            }
        }
        /// Creates an enum from field names used in the ProtoBuf definition.
        pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
            match value {
                "DA_PENDING" => Some(Self::DaPending),
                "DA_AVALIBLE" => Some(Self::DaAvalible),
                "ZKP_PENDING" => Some(Self::ZkpPending),
                "ZKP_FINISHED" => Some(Self::ZkpFinished),
                "RETRYABLE_FAILURE" => Some(Self::RetryableFailure),
                "PERMANENT_FAILURE" => Some(Self::PermanentFailure),
                _ => None,
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum ResponseValue {
        /// When ZKP_PENDING, this is the proof request/job id on the prover network
        #[prost(bytes, tag = "2")]
        ProofId(::prost::alloc::vec::Vec<u8>),
        /// When ZKP_FINISHED, this is the proof data
        #[prost(bytes, tag = "3")]
        Proof(::prost::alloc::vec::Vec<u8>),
        /// Used when status is FAILED, this includes details why
        #[prost(string, tag = "4")]
        ErrorMessage(::prost::alloc::string::String),
        /// Additional details on status of a request
        #[prost(string, tag = "5")]
        StatusMessage(::prost::alloc::string::String),
    }
}
/// Generated client implementations.
pub mod inclusion_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct InclusionClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl<T> InclusionClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InclusionClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            InclusionClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn get_keccak_inclusion(
            &mut self,
            request: impl tonic::IntoRequest<super::GetKeccakInclusionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetKeccakInclusionResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/eqs.Inclusion/GetKeccakInclusion",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("eqs.Inclusion", "GetKeccakInclusion"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod inclusion_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with InclusionServer.
    #[async_trait]
    pub trait Inclusion: Send + Sync + 'static {
        async fn get_keccak_inclusion(
            &self,
            request: tonic::Request<super::GetKeccakInclusionRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetKeccakInclusionResponse>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct InclusionServer<T: Inclusion> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: Inclusion> InclusionServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for InclusionServer<T>
    where
        T: Inclusion,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/eqs.Inclusion/GetKeccakInclusion" => {
                    #[allow(non_camel_case_types)]
                    struct GetKeccakInclusionSvc<T: Inclusion>(pub Arc<T>);
                    impl<
                        T: Inclusion,
                    > tonic::server::UnaryService<super::GetKeccakInclusionRequest>
                    for GetKeccakInclusionSvc<T> {
                        type Response = super::GetKeccakInclusionResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetKeccakInclusionRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as Inclusion>::get_keccak_inclusion(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetKeccakInclusionSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: Inclusion> Clone for InclusionServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: Inclusion> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: Inclusion> tonic::server::NamedService for InclusionServer<T> {
        const NAME: &'static str = "eqs.Inclusion";
    }
}
