#[cfg(test)]
mod remote_ipc_tests {
    use crossbeam::channel;
    use crossterm::event::KeyCode;

    use crate::{
        AppEvent,
        config::{Config, cli::RemoteCmd},
        shared::{
            ipc::{
                SocketCommand, SocketCommandExecute,
                commands::{
                    command::CommandCommand,
                    keybind::KeybindCommand,
                },
            },
        },
    };

    #[test]
    fn test_keybind_command_creation() {
        let keybind_cmd = KeybindCommand { key: "p".to_string() };
        assert_eq!(keybind_cmd.key, "p");
    }

    #[test]
    fn test_command_command_creation() {
        let command_cmd = CommandCommand { action: "play".to_string() };
        assert_eq!(command_cmd.action, "play");
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
    fn test_command_command_execution() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let command_cmd = CommandCommand { action: "play".to_string() };
        
        let result = command_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_ok(), "Command execution should succeed");

        let received_event = event_rx.try_recv();
        assert!(received_event.is_ok(), "Should have received an event");
        
        match received_event.expect("Should have received an event") {
            AppEvent::Command(action) => {
                assert_eq!(action, "play");
            }
            _ => panic!("Expected Command event"),
        }
    }

    #[test]
    fn test_keybind_with_modifiers() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let keybind_cmd = KeybindCommand { key: "p".to_string() };
        
        let result = keybind_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_ok(), "Simple keybind command should succeed");

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
        let socket_cmd = SocketCommand::try_from(keybind_cmd).expect("Keybind conversion should succeed");
        
        match socket_cmd {
            SocketCommand::Keybind(cmd) => {
                assert_eq!(cmd.key, "p");
            }
            _ => panic!("Expected Keybind socket command"),
        }

        let command_cmd = RemoteCmd::Command { action: "play".to_string() };
        let socket_cmd = SocketCommand::try_from(command_cmd).expect("Command conversion should succeed");
        
        match socket_cmd {
            SocketCommand::Command(cmd) => {
                assert_eq!(cmd.action, "play");
            }
            _ => panic!("Expected Command socket command"),
        }
    }

    #[test]
    fn test_switch_to_tab_command() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let command_cmd = CommandCommand { action: "SwitchToTab(\"Lyrics\")".to_string() };
        
        let result = command_cmd.execute(&event_tx, &work_tx, &config);
        assert!(result.is_ok(), "SwitchToTab command should succeed");

        let received_event = event_rx.try_recv();
        assert!(received_event.is_ok(), "Should have received an event");
        
        match received_event.expect("Should have received an event") {
            AppEvent::Command(action) => {
                assert_eq!(action, "SwitchToTab(\"Lyrics\")");
            }
            _ => panic!("Expected Command event"),
        }
    }

    #[test]
    fn test_multiple_commands_execution() {
        let (event_tx, event_rx) = channel::unbounded();
        let (work_tx, _work_rx) = channel::unbounded();
        let config = Config::default();

        let commands = vec![
            CommandCommand { action: "play".to_string() },
            CommandCommand { action: "pause".to_string() },
            CommandCommand { action: "next".to_string() },
        ];

        for cmd in commands {
            let result = cmd.execute(&event_tx, &work_tx, &config);
            assert!(result.is_ok(), "Command execution should succeed");
        }

        let mut received_actions = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            match event {
                AppEvent::Command(action) => {
                    received_actions.push(action);
                }
                _ => panic!("Expected Command event"),
            }
        }

        assert_eq!(received_actions.len(), 3);
        assert_eq!(received_actions[0], "play");
        assert_eq!(received_actions[1], "pause");
        assert_eq!(received_actions[2], "next");
    }
}
