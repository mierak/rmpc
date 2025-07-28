#[cfg(test)]
mod remote_ipc_tests {
    use crossbeam::channel::{self, Receiver, Sender};
    use crossterm::event::KeyCode;

    use crate::{
        AppEvent,
        WorkRequest,
        config::{Config, ConfigFile, cli::RemoteCmd},
        shared::ipc::{
            SocketCommand,
            SocketCommandExecute,
            commands::{keybind::KeybindCommand, switch_tab::SwitchTabCommand},
        },
        tests::fixtures::ipc_stream,
    };

    fn setup_test() -> (Sender<AppEvent>, Receiver<AppEvent>, Sender<WorkRequest>, Config) {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = ConfigFile::default()
            .into_config(None, None, None, None, true)
            .expect("Failed to create test config");
        (event_tx, event_rx, work_tx, config)
    }

    fn expect_key_event(event_rx: &Receiver<AppEvent>, expected_key: char) {
        let event = event_rx.try_recv().expect("Should have received an event");
        match event {
            AppEvent::UserKeyInput(key_event) => {
                assert_eq!(key_event.code, KeyCode::Char(expected_key));
            }
            _ => panic!("Expected UserKeyInput event"),
        }
    }

    fn expect_remote_switch_tab(event_rx: &Receiver<AppEvent>, expected_tab: &str) {
        let event = event_rx.try_recv().expect("Should have received an event");
        match event {
            AppEvent::RemoteSwitchTab { tab_name } => {
                assert_eq!(tab_name, expected_tab);
            }
            _ => panic!("Expected RemoteSwitchTab event"),
        }
    }

    #[test]
    fn test_keybind_command_creation() {
        let keybind_cmd = KeybindCommand { key: "p".to_string() };
        assert_eq!(keybind_cmd.key, "p");
    }

    #[test]
    fn test_switch_tab_command_creation() {
        let switch_tab_cmd = SwitchTabCommand { tab: "Queue".to_string() };
        assert_eq!(switch_tab_cmd.tab, "Queue");
    }

    #[test]
    fn test_keybind_command_execution() {
        let (event_tx, event_rx, work_tx, config) = setup_test();
        let keybind_cmd = KeybindCommand { key: "p".to_string() };

        let result = keybind_cmd.execute(&event_tx, &work_tx, ipc_stream(), &config);
        assert!(result.is_ok());

        expect_key_event(&event_rx, 'p');
    }

    #[test]
    fn test_switch_tab_command_execution() {
        let (event_tx, event_rx, work_tx, config) = setup_test();
        let switch_tab_cmd = SwitchTabCommand { tab: "Queue".to_string() };

        let result = switch_tab_cmd.execute(&event_tx, &work_tx, ipc_stream(), &config);
        assert!(result.is_ok());

        expect_remote_switch_tab(&event_rx, "Queue");
    }

    #[test]
    fn test_switch_tab_case_insensitive() {
        let (event_tx, event_rx, work_tx, config) = setup_test();
        let test_cases = ["queue", "QUEUE", "Queue", "QuEuE"];

        for test_case in test_cases {
            let switch_tab_cmd = SwitchTabCommand { tab: test_case.to_string() };
            let result = switch_tab_cmd.execute(&event_tx, &work_tx, ipc_stream(), &config);
            assert!(result.is_ok(), "Switch tab should work case-insensitively for '{test_case}'");

            expect_remote_switch_tab(&event_rx, test_case);
        }
    }

    #[test]
    fn test_switch_tab_invalid_tab() {
        let (event_tx, event_rx, work_tx, _) = setup_test();
        let config = Config::default(); // Use minimal config for error test
        let switch_tab_cmd = SwitchTabCommand { tab: "NonExistentTab".to_string() };

        let result = switch_tab_cmd.execute(&event_tx, &work_tx, ipc_stream(), &config);
        assert!(result.is_ok(), "Command execution should always succeed at socket level");

        // Checking that a RemoteSwitchTab event was sent (since validation happens in
        // main event loop)
        expect_remote_switch_tab(&event_rx, "NonExistentTab");
    }

    #[test]
    fn test_invalid_keybind() {
        let (event_tx, _event_rx, work_tx, config) = setup_test();
        let keybind_cmd = KeybindCommand { key: "invalid_key_format".to_string() };

        let result = keybind_cmd.execute(&event_tx, &work_tx, ipc_stream(), &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_remote_cmd_to_socket_command_conversion() {
        let keybind_cmd = RemoteCmd::Keybind { key: "p".to_string() };
        let socket_cmd =
            SocketCommand::try_from(&keybind_cmd).expect("Keybind conversion should succeed");

        match socket_cmd {
            SocketCommand::Keybind(cmd) => {
                assert_eq!(cmd.key, "p");
            }
            _ => panic!("Expected Keybind socket command"),
        }

        let switch_tab_cmd = RemoteCmd::SwitchTab { tab: "Queue".to_string() };
        let socket_cmd =
            SocketCommand::try_from(&switch_tab_cmd).expect("SwitchTab conversion should succeed");

        match socket_cmd {
            SocketCommand::SwitchTab(cmd) => {
                assert_eq!(cmd.tab, "Queue");
            }
            _ => panic!("Expected SwitchTab socket command"),
        }
    }
}
