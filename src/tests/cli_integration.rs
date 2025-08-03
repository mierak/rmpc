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
    fn test_cli_switch_tab_parsing() {
        let args = Args::try_parse_from(["rmpc", "remote", "switchtab", "Queue"])
            .expect("Failed to parse CLI arguments");

        match args.command {
            Some(crate::config::cli::Command::Remote {
                command: RemoteCmd::SwitchTab { tab },
                ..
            }) => {
                assert_eq!(tab, "Queue");
            }
            _ => panic!("Expected Remote SwitchTab command"),
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
    fn test_switch_tab_variants() {
        let tab_names = vec!["Queue", "Directories", "Artists", "Albums", "Playlists", "Search"];

        for tab_name in tab_names {
            let args = Args::try_parse_from(["rmpc", "remote", "switchtab", tab_name])
                .expect("Failed to parse CLI arguments");
            match args.command {
                Some(crate::config::cli::Command::Remote {
                    command: RemoteCmd::SwitchTab { tab },
                    ..
                }) => {
                    assert_eq!(tab, tab_name);
                }
                _ => panic!("Expected Remote SwitchTab command for tab: {tab_name}"),
            }
        }
    }

    #[test]
    fn test_switch_tab_case_insensitive_parsing() {
        let test_cases = vec!["queue", "QUEUE", "Queue", "QuEuE", "directories", "ARTISTS"];

        for tab_name in test_cases {
            let args = Args::try_parse_from(["rmpc", "remote", "switchtab", tab_name])
                .unwrap_or_else(|_| panic!("Failed to parse CLI arguments for tab: {tab_name}"));
            match args.command {
                Some(crate::config::cli::Command::Remote {
                    command: RemoteCmd::SwitchTab { tab },
                    ..
                }) => {
                    assert_eq!(tab, tab_name);
                }
                _ => panic!("Expected Remote SwitchTab command for tab: {tab_name}"),
            }
        }
    }

    #[test]
    fn test_cli_with_pid() {
        let args = Args::try_parse_from(["rmpc", "remote", "--pid", "12345", "switchtab", "Queue"])
            .expect("Failed to parse CLI arguments");

        match args.command {
            Some(crate::config::cli::Command::Remote {
                command: RemoteCmd::SwitchTab { tab },
                pid,
            }) => {
                assert_eq!(tab, "Queue");
                assert_eq!(pid, Some(12345));
            }
            _ => panic!("Expected Remote SwitchTab command with PID"),
        }
    }
}
