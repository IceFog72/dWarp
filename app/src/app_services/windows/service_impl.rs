use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_channel::Sender;
use async_trait::async_trait;
use ipc::{Client, ConnectionAddress};
use warp_cli::StartupRequest;
use warpui::r#async::executor::Background;

use super::single_instance_manager::uri_named_pipe_name;

/// IPC service for local startup requests sent to the running instance.
pub(super) struct StartupService {}

impl ipc::Service for StartupService {
    type Request = StartupRequest;
    type Response = Result<(), String>;
}

#[derive(Clone)]
pub(super) struct StartupServiceImpl {
    tx: Sender<StartupRequest>,
}

impl StartupServiceImpl {
    pub(super) fn new(tx: Sender<StartupRequest>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl ipc::ServiceImpl for StartupServiceImpl {
    type Service = StartupService;

    async fn handle_request(&self, mut request: StartupRequest) -> Result<(), String> {
        if let StartupRequest::Terminal(launch) = &mut request {
            crate::terminal_launch::validate(launch).map_err(|err| err.to_string())?;
        }
        self.tx.send(request).await.map_err(|err| err.to_string())
    }
}

/// Forwards a local startup request to the main running instance.
pub(super) async fn forward_startup_request_to_sole_running_instance(
    request: StartupRequest,
) -> anyhow::Result<()> {
    // We need to construct a new background executor because this function is
    // run before we have a `AppContext`.  We explicitly create it with
    // a single backing thread, as we don't need an entire pool of threads.
    let background_executor = Arc::new(Background::new(1, |_| "forward-uris".to_owned()));
    // The mutex is acquired before the GUI starts, but the pipe is created
    // during app service initialization. Wait for it without resending requests.
    let deadline = Instant::now() + Duration::from_secs(30);
    let client = loop {
        match Client::connect(
            ConnectionAddress::from(uri_named_pipe_name()),
            background_executor.clone(),
        )
        .await
        {
            Ok(client) => break client,
            Err(ipc::ClientError::Initialization(_)) if Instant::now() < deadline => {
                warpui::r#async::Timer::after(Duration::from_millis(100)).await;
            }
            Err(err) => return Err(err.into()),
        }
    };
    let caller = ipc::service_caller::<StartupService>(Arc::new(client));
    caller.call(request).await?.map_err(anyhow::Error::msg)
}
