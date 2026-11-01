use registry::register_uri_handler;
use warpui::AppContext;
use {
    service_impl::forward_startup_request_to_sole_running_instance,
    single_instance_manager::SingleInstanceManager, thiserror::Error,
};

mod registry;
mod service_impl;
mod single_instance_manager;

#[derive(Error, Debug)]
pub enum StartupArgsForwardingError {
    #[error("should not forward arguments after an auto-update")]
    IgnoredAfterAutoUpdate,
    #[error("there is no other instance of Warp")]
    NoExistingInstance,
    #[error("failed to construct startup request")]
    StartupRequest(#[from] anyhow::Error),
    #[error("Win32 error")]
    WindowsError(#[from] windows::core::Error),
}

pub fn pass_startup_args_to_existing_instance(
    args: &warp_cli::AppArgs,
) -> Result<(), StartupArgsForwardingError> {
    if args.finish_update {
        return Err(StartupArgsForwardingError::IgnoredAfterAutoUpdate);
    }
    if SingleInstanceManager::is_sole_running_instance()? {
        return Err(StartupArgsForwardingError::NoExistingInstance);
    }

    warpui::r#async::block_on(async {
        forward_startup_request_to_sole_running_instance(crate::terminal_launch::prepare_request(
            args,
        )?)
        .await?;

        Ok(())
    })
}

pub(super) fn init(ctx: &mut AppContext) {
    ctx.add_singleton_model(SingleInstanceManager::new);
    register_uri_handler();
}
