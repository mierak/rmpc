#[cfg(test)]
mod remote_ipc_tests {
    use crossbeam::channel;
    use crossterm::event::KeyCode;

    use crate::{
        AppEvent,
        config::{Config, cli::RemoteCmd},
        shared::ipc::{SocketCommand, SocketCommandExecute, commands::keybind::KeybindCommand},
    };

    #[test]
    fn test_keybind_command_creation() {
        let keybind_cmd = KeybindCommand { key: "p".to_string() };
        assert_eq!(keybind_cmd.key, "p");
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
    }
}
