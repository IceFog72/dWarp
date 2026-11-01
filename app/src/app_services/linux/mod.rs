use std::collections::HashMap;

use itertools::Itertools as _;
use parking_lot::Mutex;
use warpui::{AppContext, SingletonEntity};
use zbus::{interface, proxy, zvariant};

use crate::channel::ChannelState;

static STARTUP_SERVICE: Mutex<Option<DBusService>> = Mutex::new(None);

struct DBusService {
    connection: zbus::Connection,
    events: async_channel::Receiver<ApplicationServiceEvent>,
}

impl DBusService {
    async fn reserve() -> zbus::Result<Self> {
        let (tx, events) = async_channel::unbounded();
        let connection = zbus::connection::Builder::session()?
            .name(DBusServiceHost::well_known_name())?
            .allow_name_replacements(false)
            .replace_existing_names(false)
            .serve_at(
                DBusServiceHost::application_service_path(),
                ApplicationService { tx },
            )?
            .build()
            .await?;
        Ok(Self { connection, events })
    }
}

/// Initializes application services.
pub fn init(ctx: &mut AppContext) {
    ctx.add_singleton_model(DBusServiceHost::new);
}

/// Tears down application services.
pub fn teardown(ctx: &mut AppContext) {
    DBusServiceHost::handle(ctx).update(ctx, |service_host, _| {
        service_host.terminate();
    });
}

/// Attempts to forward startup arguments to an existing instance of the
/// application.
///
/// Returns Ok if an existing instance exists and was reachable.
pub fn pass_startup_args_to_existing_instance(
    args: &warp_cli::AppArgs,
) -> Result<(), StartupArgsForwardingError> {
    if args.finish_update {
        return Err(StartupArgsForwardingError::IgnoredAfterAutoUpdate);
    }

    warpui::r#async::block_on(async {
        // Claim the name before creating a GUI. The service queues requests
        // while the first process is still initializing its UI.
        match DBusService::reserve().await {
            Ok(service) => {
                *STARTUP_SERVICE.lock() = Some(service);
                return Err(StartupArgsForwardingError::NoExistingInstance);
            }
            Err(zbus::Error::NameTaken) => {}
            Err(err) => return Err(err.into()),
        }

        let conn = zbus::Connection::session().await?;
        let proxy = ExistingApplicationProxy::builder(&conn)
            .destination(DBusServiceHost::well_known_name())?
            .path(DBusServiceHost::application_service_path())?
            .build()
            .await?;
        let urls = args.startup_urls()?;
        let url_refs = urls.iter().map(AsRef::as_ref).collect_vec();
        proxy.open(&url_refs, HashMap::new()).await?;

        // Make sure we close the connection and clean up resources, to avoid
        // leaving behind file descriptors that will interfere with the terminal
        // server spawn process.
        let _ = conn.close().await;

        Ok(())
    })
}

#[derive(Debug, thiserror::Error)]
pub enum StartupArgsForwardingError {
    /// There's no instance of Warp already running.
    #[error("no existing instance found to forward args to")]
    NoExistingInstance,
    /// This instance was launched after an auto-update and should not forward
    /// arguments to the old (terminating) instance.
    #[error("should not forward args after an auto-update")]
    IgnoredAfterAutoUpdate,
    /// An unknown D-Bus error occurred.
    #[error("unknown dbus error")]
    Unknown(zbus::Error),
    #[error("failed to construct startup request")]
    StartupRequest(#[from] anyhow::Error),
}

impl From<zbus::fdo::Error> for StartupArgsForwardingError {
    fn from(value: zbus::fdo::Error) -> Self {
        // While ServiceUnknown usually means that D-Bus doesn't know how to
        // _launch_ something to handle your message, in our case, we're not
        // registering a service, so this really means that Warp is not already
        // running.
        if matches!(value, zbus::fdo::Error::ServiceUnknown(_)) {
            StartupArgsForwardingError::NoExistingInstance
        } else {
            StartupArgsForwardingError::Unknown(zbus::Error::FDO(Box::new(value)))
        }
    }
}

impl From<zbus::Error> for StartupArgsForwardingError {
    fn from(value: zbus::Error) -> Self {
        match value {
            zbus::Error::FDO(err) => (*err).into(),
            err => StartupArgsForwardingError::Unknown(err),
        }
    }
}

enum ApplicationServiceEvent {
    Open { uris: Vec<String> },
}

/// A structure providing an implementation of the org.freedesktop.Application
/// D-Bus service.
struct ApplicationService {
    tx: async_channel::Sender<ApplicationServiceEvent>,
}

/// An implementation of the org.freedesktop.Application D-Bus service.
///
/// See: https://specifications.freedesktop.org/desktop-entry-spec/1.5/ar01s08.html
#[interface(name = "org.freedesktop.Application")]
impl ApplicationService {
    /// Called when the application is started without any files to open.
    async fn activate(
        &self,
        _platform_data: HashMap<String, zvariant::Value<'_>>,
    ) -> zbus::fdo::Result<()> {
        let uris = warp_cli::AppArgs::default()
            .startup_urls()
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?
            .into_iter()
            .map(String::from)
            .collect();
        self.tx
            .send(ApplicationServiceEvent::Open { uris })
            .await
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))
    }

    /// Called when desktop actions are activated.
    ///
    /// See: https://specifications.freedesktop.org/desktop-entry-spec/1.5/ar01s11.html
    async fn activate_action(
        &self,
        _action_name: String,
        _parameter: Vec<zvariant::Value<'_>>,
        _platform_data: HashMap<String, zvariant::Value<'_>>,
    ) -> zbus::fdo::Result<()> {
        // not yet implemented
        Ok(())
    }

    /// Called when the application is started with files.
    async fn open(
        &self,
        uris: Vec<String>,
        _platform_data: HashMap<String, zvariant::Value<'_>>,
    ) -> zbus::fdo::Result<()> {
        let _ = self.tx.send(ApplicationServiceEvent::Open { uris }).await;
        Ok(())
    }
}

// A D-Bus client for connecting to an already-running instance of Warp and
// invoking org.freedesktop.Application IPC methods.
#[proxy(
    interface = "org.freedesktop.Application",
    default_service = "dev.warp.WarpLocal",
    default_path = "/dev/warp/WarpLocal",
    gen_blocking = false
)]
trait ExistingApplication {
    fn activate(&self, platform_data: HashMap<&str, zvariant::Value<'_>>) -> zbus::fdo::Result<()>;

    fn activate_action(
        &self,
        action_name: &str,
        parameter: &[zvariant::Value<'_>],
        platform_data: HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::fdo::Result<()>;

    fn open(
        &self,
        uris: &[&str],
        platform_data: HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::fdo::Result<()>;
}

/// A singleton model that is responsible for hosting all D-Bus services
/// exposed by the application.
struct DBusServiceHost {
    connection: Option<zbus::Connection>,
}

impl DBusServiceHost {
    fn new(ctx: &mut warpui::ModelContext<Self>) -> Self {
        let service = STARTUP_SERVICE
            .lock()
            .take()
            .map(Ok)
            .unwrap_or_else(|| warpui::r#async::block_on(DBusService::reserve()));
        let DBusService { connection, events } = match service {
            Ok(service) => service,
            Err(err) => {
                log::error!(
                    "Failed to initialize org.freedesktop.Application D-Bus service: {err:#}"
                );
                return Self { connection: None };
            }
        };

        // Process any events that we receive over D-Bus.
        ctx.spawn_stream_local(events, |_, event, ctx| {
            match event {
                ApplicationServiceEvent::Open { uris } => {
                    for uri in uris {
                        match url::Url::parse(&uri) {
                            Ok(uri) => crate::uri::handle_incoming_uri(&uri, ctx),
                            Err(err) => log::warn!("Failed to parse URI when handling org.freedesktop.Application/open: {err:#}"),
                        }
                    }
                },
            }
        }, |_, _| {});

        Self {
            connection: Some(connection),
        }
    }

    fn terminate(&mut self) {
        if let Some(connection) = self.connection.take() {
            if let Err(err) = warpui::r#async::block_on(connection.close()) {
                log::warn!("Failed to close application D-Bus service: {err:#}");
            }
        }
    }

    /// Returns the D-Bus well-known name that should be used.
    fn well_known_name() -> String {
        format!(
            "{}{}",
            ChannelState::app_id(),
            super::instance_profile_suffix()
        )
    }

    /// Returns the path under which the org.freedesktop.Application interface
    /// will be hosted.
    fn application_service_path() -> String {
        format!("/{}", Self::well_known_name().split('.').join("/"))
    }
}

impl warpui::Entity for DBusServiceHost {
    type Event = ();
}

impl SingletonEntity for DBusServiceHost {}
