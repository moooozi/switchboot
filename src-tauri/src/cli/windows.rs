const SERVICE_NAME: &str = "swboot-cli";
const SERVICE_DISPLAY_NAME: &str = "Switchboot Background service";
const PIPE_NAME: &str = r"\\.\pipe\ca9ba1f9-4aaa-486f-8ce4-f69453af0c6c";

use crate::logic::{dispatch_command, CliCommand, CommandResponse};
use std::sync::Arc;
use winservice_ipc::{pipe_server, run_service, IPC};
use winservice_ipc::{ClientRequest, ServerResponse};

#[cfg(windows)]
pub fn launch_windows_service() {
    winservice_ipc::run_windows_service(SERVICE_NAME, my_service_main);
}

#[cfg(windows)]
pub fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    println!("Service main started with arguments: {:?}", arguments);
    let pipe_name_owned = PIPE_NAME.to_owned();
    if let Err(e) = run_service(SERVICE_NAME, move |ctx| {
        let ipc = Arc::new(IPC::new(&pipe_name_owned));
        ipc.set_non_blocking();
        pipe_server(ctx.stop_flag, ipc, handle_client_request);
    }) {
        println!("Error running service: {:?}", e);
    }
}

#[cfg(windows)]
fn handle_client_request(ipc: &IPC, request: &[u8]) {
    // Deserialize the request using bincode

    let client_req: Result<ClientRequest, _> = bincode::deserialize(request);
    let response = match client_req {
        Ok(req) => {
            let command: CliCommand = bincode::deserialize(&req.payload).unwrap();
            let result = dispatch_command(command);
            let result = bincode::serialize(&result).unwrap_or_default();
            ServerResponse {
                id: req.id,
                status: "ok".to_string(),
                result: Some(result),
                error: None,
            }
        }
        Err(e) => ServerResponse {
            id: "".to_string(),
            status: "error".to_string(),
            result: None,
            error: Some(format!("Deserialization error: {}", e)),
        },
    };

    // Serialize the response and send it back
    if let Ok(resp_bytes) = bincode::serialize(&response) {
        ipc.send_message(&resp_bytes);
    }
}

/// Try to send the command to the Windows service via IPC.
/// Returns Some(CommandResponse) if successful, None if IPC fails.
#[cfg(windows)]
fn try_ipc_command(command: &CliCommand) -> Option<CommandResponse> {
    use rand::Rng;
    use winservice_ipc::{ClientRequest, IPCClient, ServerResponse};
    let client = IPCClient::connect(PIPE_NAME).ok()?;
    let payload = bincode::serialize(command).ok()?;

    let req = ClientRequest {
        // This is the new rand syntax since rand 0.9
        id: rand::rng().random::<u128>().to_string(),
        payload,
    };
    let req_bytes = bincode::serialize(&req).ok()?;
    let resp: ServerResponse = client.send_request(req_bytes).ok()?;
    if resp.status == "ok" {
        if let Some(result_bytes) = resp.result {
            bincode::deserialize(&result_bytes).ok()
        } else {
            None
        }
    } else {
        Some(CommandResponse {
            code: 1,
            message: resp.error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }
}

#[cfg(windows)]
pub fn install_service() {
    // the current executable path
    let executable_path = std::env::current_exe().expect("Failed to get current executable path");
    let executable_path_str = executable_path
        .to_str()
        .expect("Executable path is not valid UTF-8");
    let bin_path = format!("\"{}\" /service", executable_path_str);
    winservice_ipc::install_service(SERVICE_NAME, SERVICE_DISPLAY_NAME, &bin_path).unwrap();
}

#[cfg(windows)]
pub fn uninstall_service() {
    winservice_ipc::uninstall_service(SERVICE_NAME).unwrap();
}
