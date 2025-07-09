#[cfg(test)]
mod remote_ipc_tests {
    use crossbeam::channel;
    use crossterm::event::KeyCode;

    use crate::{
        AppEvent,
        config::{Config, ConfigFile, cli::RemoteCmd, tabs::TabName},
        shared::ipc::{
            SocketCommand,
            SocketCommandExecute,
            commands::{keybind::KeybindCommand, switch_tab::SwitchTabCommand},
        },
        ui::UiAppEvent,
    };

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
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let keybind_cmd = KeybindCommand { key: "p".to_string() };

        let result = keybind_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_ok(), "Keybind command execution should succeed");

        let received_event = event_rx.try_recv();
        assert!(received_event.is_ok(), "Should have received an event");

        match received_event.expect("Should have received an event") {
            AppEvent::UserKeyInput(key_event) => {
                assert_eq!(key_event.code, KeyCode::Char('p'));
            }
            _ => panic!("Expected UserKeyInput event"),
        }
    }

    #[test]
    fn test_switch_tab_command_execution() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();

        let config_file = ConfigFile::default();
        let config = config_file
            .into_config(None, None, None, None, true)
            .expect("Failed to create config from default config file");

        let switch_tab_cmd = SwitchTabCommand { tab: "Queue".to_string() };

        let result = switch_tab_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_ok(), "SwitchTab command execution should succeed");

        let received_event = event_rx.try_recv();
        assert!(received_event.is_ok(), "Should have received an event");

        match received_event.expect("Should have received an event") {
            AppEvent::UiEvent(UiAppEvent::ChangeTab(tab_name)) => {
                assert_eq!(tab_name, TabName::from("Queue".to_string()));
            }
            _ => panic!("Expected UiEvent::ChangeTab event"),
        }
    }

    #[test]
    fn test_switch_tab_case_insensitive() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();

        let config_file = ConfigFile::default();
        let config = config_file
            .into_config(None, None, None, None, true)
            .expect("Failed to create config from default config file");

        let test_cases = vec!["queue", "QUEUE", "Queue", "QuEuE"];

        for test_case in test_cases {
            let switch_tab_cmd = SwitchTabCommand { tab: test_case.to_string() };
            let result = switch_tab_cmd.execute(&event_tx, &work_tx, &config);
            assert!(
                result.is_ok(),
                "Switch tab should work case-insensitively for '{test_case}'"
            );

            let received_event = event_rx.try_recv();
            assert!(received_event.is_ok(), "Should have received an event for '{test_case}'");

            match received_event.expect("Should have received an event") {
                AppEvent::UiEvent(UiAppEvent::ChangeTab(tab_name)) => {
                    // this should always resolve to the correct case "Queue"
                    assert_eq!(tab_name, TabName::from("Queue".to_string()));
                }
                _ => panic!("Expected UiEvent::ChangeTab event for '{test_case}'"),
            }
        }
    }

    #[test]
    fn test_switch_tab_invalid_tab() {
        let (event_tx, _event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let switch_tab_cmd = SwitchTabCommand { tab: "NonExistentTab".to_string() };

        let result = switch_tab_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_err(), "Invalid tab name should fail");
    }

    #[test]
    fn test_invalid_keybind() {
        let (event_tx, _event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let keybind_cmd = KeybindCommand { key: "invalid_key_format".to_string() };

        let result = keybind_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_err(), "Invalid keybind should fail");
    }

    #[test]
    fn test_remote_cmd_to_socket_command_conversion() {
        let keybind_cmd = RemoteCmd::Keybind { key: "p".to_string() };
        let socket_cmd =
            SocketCommand::try_from(keybind_cmd).expect("Keybind conversion should succeed");

        match socket_cmd {
            SocketCommand::Keybind(cmd) => {
                assert_eq!(cmd.key, "p");
            }
            _ => panic!("Expected Keybind socket command"),
        }

        let switch_tab_cmd = RemoteCmd::SwitchTab { tab: "Queue".to_string() };
        let socket_cmd =
            SocketCommand::try_from(switch_tab_cmd).expect("SwitchTab conversion should succeed");

        match socket_cmd {
            SocketCommand::SwitchTab(cmd) => {
                assert_eq!(cmd.tab, "Queue");
            }
            _ => panic!("Expected SwitchTab socket command"),
        }
    }
}
