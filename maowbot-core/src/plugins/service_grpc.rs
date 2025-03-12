// src/plugins/service_grpc.rs
use std::pin::Pin;
use std::sync::Arc;

use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status};

use maowbot_proto::plugs::{
    plugin_service_server::{PluginService},
    PluginStreamRequest, PluginStreamResponse,
};

use crate::plugins::manager::PluginManager;

/// A Tonic gRPC service that wraps the `PluginManager`.
#[derive(Clone)]
pub struct PluginServiceGrpc {
    pub manager: Arc<PluginManager>,
}

/// The type alias for our server streaming of `PluginStreamResponse`.
type SessionStream = Pin<Box<dyn Stream<Item = Result<PluginStreamResponse, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl PluginService for PluginServiceGrpc {
    type StartSessionStream = SessionStream;

    /// Called by a plugin that initiates a new gRPC stream.
    /// We hold the inbound stream, attach it to the manager, and produce an outbound stream.
    async fn start_session(
        &self,
        request: Request<tonic::Streaming<PluginStreamRequest>>,
    ) -> Result<Response<Self::StartSessionStream>, Status> {
        tracing::info!("PluginServiceGrpc::start_session => new plugin stream connected.");

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginStreamResponse>();
        let mgr = self.manager.clone();

        // Let the manager handle incoming messages:
        mgr.handle_new_grpc_stream(request.into_inner(), tx).await;

        // Then produce the outgoing unbounded stream:
        let out_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(Ok);
        let pinned: SessionStream = Box::pin(out_stream);

        Ok(Response::new(pinned))
    }
}