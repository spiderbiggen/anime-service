use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;
use tracing::warn;

use crate::models::DownloadGroup;

pub(crate) struct DownloadService {
    pub(crate) sender: Sender<DownloadGroup>,
}

#[tonic::async_trait]
impl proto::api::v2::downloads_server::Downloads for DownloadService {
    type SubscribeStream = ReceiverStream<Result<proto::api::v2::DownloadCollection, Status>>;
    async fn subscribe(
        &self,
        request: tonic::Request<()>,
    ) -> Result<tonic::Response<Self::SubscribeStream>, Status> {
        let mut incoming = self.sender.subscribe();
        let (tx, rx) = mpsc::channel(3);
        tokio::spawn(async move {
            loop {
                match incoming.recv().await {
                    Ok(group) => {
                        if tx.send(Ok(group.into())).await.is_err() {
                            warn!(
                                "failed to push downloads to client at {:?}",
                                request.remote_addr()
                            );
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("failed to receive new episode from shared sender: {e}");
                        let message = Err(Status::unavailable(e.to_string()));
                        if tx.send(message).await.is_err() {
                            warn!(
                                "failed to push error to client at {:?}",
                                request.remote_addr()
                            );
                        }
                        break;
                    }
                }
            }
        });

        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}
