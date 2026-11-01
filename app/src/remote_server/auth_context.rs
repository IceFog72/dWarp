use remote_server::auth::RemoteServerAuthContext;

/// SSH authenticates the remote account. Partition its daemon files by application.
pub fn local_auth_context() -> RemoteServerAuthContext {
    RemoteServerAuthContext::new(|| Box::pin(async { None }), || "openwarp".to_owned())
}
