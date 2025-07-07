#[cfg(test)]
mod cli_integration_tests {
    use clap::Parser;

    use crate::config::cli::{Args, RemoteCmd};

    #[test]
    fn test_cli_keybind_parsing() {
        let args = Args::try_parse_from(["rmpc", "remote", "keybind", "p"])
            .expect("Failed to parse CLI arguments");

        match args.command {
            Some(crate::config::cli::Command::Remote {
                command: RemoteCmd::Keybind { key },
                ..
            }) => {
                assert_eq!(key, "p");
            }
            _ => panic!("Expected Remote Keybind command"),
        }
    }

    #[test]
    fn test_cli_command_parsing() {
        let args = Args::try_parse_from(["rmpc", "remote", "command", "play"])
            .expect("Failed to parse CLI arguments");

        match args.command {
            Some(crate::config::cli::Command::Remote {
                command: RemoteCmd::Command { action },
                ..
            }) => {
                assert_eq!(action, "play");
            }
            _ => panic!("Expected Remote Command command"),
        }
    }

    #[test]
    fn test_keybind_key_variants() {
        let key_inputs = vec!["p", "ctrl+p", "shift+p", "alt+p", "Enter", "Escape", "Space"];

        for input in key_inputs {
            let args = Args::try_parse_from(["rmpc", "remote", "keybind", input])
                .expect("Failed to parse CLI arguments");
            match args.command {
                Some(crate::config::cli::Command::Remote {
                    command: RemoteCmd::Keybind { key },
                    ..
                }) => {
                    assert_eq!(key, input);
                }
                _ => panic!("Expected Remote Keybind command for input: {input}"),
            }
        }
    }

    #[test]
    fn test_command_action_variants() {
        let actions = vec!["play", "pause", "next", "prev", "SwitchToTab(\"Lyrics\")", "volume 50"];

        for action in actions {
            let args = Args::try_parse_from(["rmpc", "remote", "command", action])
                .expect("Failed to parse CLI arguments");
            match args.command {
                Some(crate::config::cli::Command::Remote {
                    command: RemoteCmd::Command { action: parsed_action },
                    ..
                }) => {
                    assert_eq!(parsed_action, action);
                }
                _ => panic!("Expected Remote Command command for action: {action}"),
            }
        }
    }

    #[test]
    fn test_cli_with_pid() {
        let args = Args::try_parse_from(["rmpc", "remote", "--pid", "12345", "keybind", "p"])
            .expect("Failed to parse CLI arguments");

        match args.command {
            Some(crate::config::cli::Command::Remote {
                command: RemoteCmd::Keybind { key },
                pid,
            }) => {
                assert_eq!(key, "p");
                assert_eq!(pid, Some(12345));
            }
            _ => panic!("Expected Remote Keybind command with PID"),
        }
    }
}
