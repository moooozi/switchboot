mod winservice;
mod ipc_server;
mod ipc_client;
mod ipc_messaging;

pub use winservice::{
    run_windows_service,
    run_service,
    install_service,
    uninstall_service
};
pub use ipc_server::IPC;
pub use ipc_messaging::{pipe_server, ClientRequest, ServerResponse};
pub use ipc_client::IPCClient;
