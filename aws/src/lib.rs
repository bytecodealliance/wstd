use anyhow::anyhow;
use aws_smithy_async::rt::sleep::{AsyncSleep, Sleep};
use aws_smithy_runtime_api::client::http::{
    HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpConnector,
};
use aws_smithy_runtime_api::client::orchestrator::HttpRequest;
use aws_smithy_runtime_api::client::result::ConnectorError;
use aws_smithy_runtime_api::client::retries::ErrorKind;
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
use aws_smithy_runtime_api::http::Response;
use aws_smithy_types::body::SdkBody;
use http_body_util::{BodyStream, StreamBody};
use std::time::Duration;
use sync_wrapper::SyncStream;
use wstd::http::{Body as WstdBody, BodyExt, Client};

pub fn sleep_impl() -> impl AsyncSleep + 'static {
    WstdSleep
}

#[derive(Debug)]
struct WstdSleep;
impl AsyncSleep for WstdSleep {
    fn sleep(&self, duration: Duration) -> Sleep {
        Sleep::new(async move {
            wstd::task::sleep(wstd::time::Duration::from(duration)).await;
        })
    }
}

pub fn http_client() -> impl HttpClient + 'static {
    WstdHttpClient
}

#[derive(Debug)]
struct WstdHttpClient;

impl HttpClient for WstdHttpClient {
    fn http_connector(
        &self,
        settings: &HttpConnectorSettings,
        // afaict, none of these components are relevant to this
        // implementation.
        _components: &RuntimeComponents,
    ) -> SharedHttpConnector {
        let mut client = Client::new();
        if let Some(timeout) = settings.connect_timeout() {
            client.set_connect_timeout(timeout);
        }
        if let Some(timeout) = settings.read_timeout() {
            client.set_first_byte_timeout(timeout);
        }
        SharedHttpConnector::new(WstdHttpConnector(client))
    }
}

#[derive(Debug)]
struct WstdHttpConnector(Client);

impl HttpConnector for WstdHttpConnector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let client = self.0.clone();
        HttpConnectorFuture::new(async move {
            let request = request
                .try_into_http1x()
                // This can only fail if the Extensions fail to convert
                .map_err(|e| ConnectorError::other(Box::new(e), None))?;
            // smithy's SdkBody Error is a non-'static boxed dyn stderror.
            // Anyhow can't represent that, so convert it to the debug impl.
            let request =
                request.map(|body| WstdBody::from_http_body(body.map_err(|e| anyhow!("{e:?}"))));
            // Any error given by send is considered a "ClientError" kind
            // which should prevent smithy from retrying like it would for a
            // throttling error
            let response = client
                .send(request)
                .await
                .map_err(|e| ConnectorError::other(e.into(), Some(ErrorKind::ClientError)))?;

            Response::try_from(response.map(|wstd_body| {
                // You'd think that an SdkBody would just be an impl Body with
                // the usual error type dance.
                let nonsync_body = wstd_body
                    .into_boxed_body()
                    .map_err(|e| e.into_boxed_dyn_error());
                // But we have to do this weird dance: because Axum insists
                // bodies are not Sync, wstd settled on non-Sync bodies.
                // Smithy insists on Sync bodies. The SyncStream type exists
                // to assert, because all Stream operations are on &mut self,
                // all Streams are Sync. So, turn the Body into a Stream, make
                // it sync, then back to a Body.
                let nonsync_stream = BodyStream::new(nonsync_body);
                let sync_stream = SyncStream::new(nonsync_stream);
                let sync_body = StreamBody::new(sync_stream);
                SdkBody::from_body_1_x(sync_body)
            }))
            // This can only fail if the Extensions fail to convert
            .map_err(|e| ConnectorError::other(Box::new(e), None))
        })
    }
}
