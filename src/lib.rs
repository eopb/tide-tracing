use std::error::Error;

use tide::{Middleware, Next, Request};
use tracing::{error, error_span, info, info_span, span, warn, Level};
use tracing_futures::Instrument;

/// Log all incoming requests and responses with tracing spans.
///
/// ```
/// let mut app = tide::Server::new();
/// app.middleware(tide_tracing::TraceMiddleware::new());
/// ```
#[derive(Debug, Default, Clone)]
pub struct TraceMiddleware;

impl TraceMiddleware {
    /// Create a new instance of `TraceMiddleware`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Log a request and a response.
    async fn log<'a, State: Clone + Send + Sync + 'static>(
        &'a self,
        ctx: Request<State>,
        next: Next<'a, State>,
    ) -> tide::Result {
        let path = ctx.url().path().to_owned();
        let method = ctx.method().to_string();

        let response = async {
            info!(method = method.as_str());

            let start = std::time::Instant::now();
            let response = next.run(ctx).await;
            let status = response.status();

            async {
                info!(status = status as u16);
                info!(duration = &format!("{:?}", start.elapsed()).as_str());

                if status.is_server_error() {
                    async {
                        if let Some(error) = response.error() {
                            let error: &(dyn Error) = error.as_ref();
                            error!(error);
                        }
                        error!("Response sent");
                    }
                    .instrument(error_span!("Internal error"))
                    .await;
                } else if status.is_client_error() {
                    let span = span!(Level::WARN, "Client Error");
                    let _enter = span.enter();

                    warn!("Client Error: Response sent");
                } else {
                    info!("Response sent");
                }
            }
            .instrument(info_span!("Response"))
            .await;
            response
        }
        .instrument(info_span!("Request", path = path.as_str()))
        .await;
        Ok(response)
    }
}

#[async_trait::async_trait]
impl<State: Clone + Send + Sync + 'static> Middleware<State> for TraceMiddleware {
    async fn handle(&self, req: Request<State>, next: Next<'_, State>) -> tide::Result {
        self.log(req, next).await
    }
}
