use crate::types::CliCommand;

impl CliCommand {
    pub const GET_BOOT_ORDER: &'static str = "get-boot-order";
    pub const SET_BOOT_ORDER: &'static str = "set-boot-order";
    pub const GET_BOOT_NEXT: &'static str = "get-boot-next";
    pub const SET_BOOT_NEXT: &'static str = "set-boot-next";
    pub const GET_BOOT_ENTRIES: &'static str = "get-boot-entries";
    pub const SAVE_BOOT_ORDER: &'static str = "save-boot-order";
    pub const UNSET_BOOT_NEXT: &'static str = "unset-boot-next";
    pub const GET_BOOT_CURRENT: &'static str = "get-boot-current";

    /// Returns true if this command can be executed in non-interactive mode via --exec
    pub fn allow_non_interactive_exec(&self) -> bool {
        match self {
            CliCommand::SetBootNext(_) => true,
            // Add other commands here in the future as needed
            _ => false,
        }
    }

    pub fn allow_non_auth_exec(&self) -> bool {
        match self {
            CliCommand::SetBootNext(_) => true,
            CliCommand::UnsetBootNext => true,
            _ => false,
        }
    }

    pub fn to_args(&self) -> Vec<String> {
        match self {
            CliCommand::GetBootOrder => vec![Self::GET_BOOT_ORDER.into()],
            CliCommand::SetBootOrder(order) => {
                let mut args = vec![Self::SET_BOOT_ORDER.into()];
                args.extend(order.iter().map(u16::to_string));
                args
            }
            CliCommand::GetBootNext => vec![Self::GET_BOOT_NEXT.into()],
            CliCommand::SetBootNext(id) => vec![Self::SET_BOOT_NEXT.into(), id.to_string()],
            CliCommand::GetBootEntries => vec![Self::GET_BOOT_ENTRIES.into()],
            CliCommand::SaveBootOrder(order) => {
                let mut args = vec![Self::SAVE_BOOT_ORDER.into()];
                args.extend(order.iter().map(u16::to_string));
                args
            }
            CliCommand::UnsetBootNext => vec![Self::UNSET_BOOT_NEXT.into()],
            CliCommand::GetBootCurrent => vec![Self::GET_BOOT_CURRENT.into()],
            CliCommand::Unknown => vec![],
        }
    }

    pub fn from_args(args: &[String]) -> Result<Self, String> {
        if args.is_empty() {
            return Ok(CliCommand::Unknown);
        }
        match args[0].as_str() {
            Self::GET_BOOT_ORDER => Ok(CliCommand::GetBootOrder),
            Self::SET_BOOT_ORDER => Ok(CliCommand::SetBootOrder(parse_u16_vec(&args[1..])?)),
            Self::GET_BOOT_NEXT => Ok(CliCommand::GetBootNext),
            Self::SET_BOOT_NEXT => match args.get(1) {
                Some(id) => Ok(CliCommand::SetBootNext(
                    id.parse::<u16>().map_err(|e| format!("Invalid u16: {e}"))?,
                )),
                None => Err("set-boot-next requires exactly one argument".to_string()),
            },
            Self::GET_BOOT_ENTRIES => Ok(CliCommand::GetBootEntries),
            Self::SAVE_BOOT_ORDER => Ok(CliCommand::SaveBootOrder(parse_u16_vec(&args[1..])?)),
            Self::UNSET_BOOT_NEXT => Ok(CliCommand::UnsetBootNext),
            Self::GET_BOOT_CURRENT => Ok(CliCommand::GetBootCurrent),
            _ => Ok(CliCommand::Unknown),
        }
    }
}

fn parse_u16_vec(slice: &[String]) -> Result<Vec<u16>, String> {
    slice
        .iter()
        .map(|s| s.parse::<u16>().map_err(|e| format!("Invalid u16: {e}")))
        .collect()
}
