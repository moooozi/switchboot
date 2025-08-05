use switchboot_lib::types::CliCommand;

/// Represents the different types of application modes/commands
#[derive(Debug, Clone)]
pub enum AppMode {
    /// Run the GUI normally
    Gui,
    /// Execute a command with optional reboot
    Exec { 
        command: CliCommand, 
        should_reboot: bool 
    },
}

/// Configuration parsed from command line arguments
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    pub mode: AppMode,
    #[cfg(target_os = "windows")]
    pub portable_mode: bool,
}

/// Parse command line arguments into application configuration
pub fn parse_args<I>(args: I) -> Result<ParsedArgs, String>
where
    I: Iterator<Item = String>,
{
    let args: Vec<String> = args.collect();
    
    // Check for portable mode (Windows only)
    #[cfg(target_os = "windows")]
    let portable_mode = args.iter().any(|arg| arg == "--portable");
    
    // Check for exec mode
    if let Some(exec_pos) = args.iter().position(|arg| arg == "--exec") {
        let remaining_args = &args[exec_pos + 1..];
        
        // Parse the command using CliCommand::from_args
        let command = CliCommand::from_args(remaining_args)
            .map_err(|e| format!("Invalid command in --exec mode: {}", e))?;
        
        // Check if this command is allowed in non-interactive exec mode
        if !command.allow_non_interactive_exec() {
            return Err(format!("Command {:?} is not allowed in --exec mode", command));
        }
        
        // Check for reboot flag
        let should_reboot = remaining_args.iter().any(|arg| arg == "reboot");
        
        return Ok(ParsedArgs {
            mode: AppMode::Exec { command, should_reboot },
            #[cfg(target_os = "windows")]
            portable_mode,
        });
    }
    
    // Default to GUI mode
    Ok(ParsedArgs {
        mode: AppMode::Gui,
        #[cfg(target_os = "windows")]
        portable_mode,
    })
}

/// Helper function to handle exec mode execution
pub fn handle_exec_mode(command: &CliCommand, should_reboot: bool) -> Result<(), String> {
    match command {
        CliCommand::SetBootNext(entry_id) => {
            switchboot_lib::handle_bootnext_shortcut_execution(*entry_id, should_reboot)
        }
        _ => Err(format!("Command {:?} is not supported in exec mode", command)),
    }
}