mod ipc_client;
mod ipc_messaging;
mod ipc_server;
mod winservice;

pub use ipc_client::IPCClient;
pub use ipc_messaging::{pipe_server, ClientRequest, ServerResponse};
pub use ipc_server::IPC;
pub use winservice::{
    install_service, run_service, run_windows_service, start_service, uninstall_service,
};
