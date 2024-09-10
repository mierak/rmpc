use crate::utils::env::ENV;

use super::utils::tilde_expand;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MpdAddress<'a> {
    IpAndPort(&'a str),
    SocketPath(&'a str),
}

impl<'a> Default for MpdAddress<'a> {
    fn default() -> Self {
        Self::IpAndPort("127.0.0.1:6600")
    }
}

impl MpdAddress<'static> {
    pub fn resolve(from_cli: Option<String>, from_config: String) -> MpdAddress<'static> {
        let mut result = from_config;

        let mpd_host = ENV.var_os("MPD_HOST");
        let mpd_host = mpd_host.as_ref().and_then(|v| v.to_str());
        let mpd_port = ENV.var_os("MPD_PORT");
        let mpd_port = mpd_port.as_ref().and_then(|v| v.to_str());

        match (mpd_host, mpd_port) {
            (Some(host), Some(port)) => {
                let expanded = tilde_expand(host);
                if expanded.starts_with('/') {
                    result = expanded.into_owned();
                } else {
                    result = format!("{host}:{port}");
                }
            }
            (Some(host), None) => {
                let expanded = tilde_expand(host);
                if expanded.starts_with('/') {
                    result = expanded.into_owned();
                } else if !expanded.starts_with('~') {
                    result = format!("{host}:6600");
                }
            }
            (None, Some(_)) | (None, None) => {}
        }

        if let Some(from_cli) = from_cli {
            result = from_cli;
        }

        if let Some((_ip, _port)) = result.split_once(':') {
            Self::IpAndPort(result.leak())
        } else {
            Self::SocketPath(result.leak())
        }
    }
}

#[cfg(test)]
#[rustfmt::skip]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use test_case::test_case;
    use crate::utils::env::ENV;
    use super::MpdAddress;

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    //               CLI Arg                 Config            MPD_HOST          MPD_PORT                    Expected                          Description
    #[test_case(Some("127.0.0.1:6600"), "127.0.0.1:7600", Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("127.0.0.1:6600")     ; "prefer CLI over all")]
    #[test_case(                  None, "127.0.0.1:7600", Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("192.168.0.1:6601")   ; "prefer ENV over config")]
    #[test_case(                  None, "127.0.0.1:7600",                None, Some("6601"), MpdAddress::IpAndPort("127.0.0.1:7600")     ; "use config when only MPD_PORT")]
    #[test_case(Some("127.0.0.1:6600"), "127.0.0.1:7600",                None,         None, MpdAddress::IpAndPort("127.0.0.1:6600")     ; "prefer CLI over config")]
    #[test_case(                  None, "127.0.0.1:7600", Some("/tmp/socket"),         None, MpdAddress::SocketPath("/tmp/socket")       ; "assume socket path when only MPD_HOST")]
    #[test_case(                  None, "127.0.0.1:7600",    Some("~/socket"),         None, MpdAddress::SocketPath("/home/u123/socket") ; "assume socket path when only MPD_HOST with tilde")]
    #[test_case(                  None, "127.0.0.1:7600", Some("192.168.0.1"),         None, MpdAddress::IpAndPort("192.168.0.1:6600")   ; "use 6600 as default port with only MPD_HOST")]
    #[test_case(                  None, "127.0.0.1:7600", Some("/tmp/socket"), Some("6601"), MpdAddress::SocketPath("/tmp/socket")       ; "assume socket path with both MPD_HOST and MPD_PORT")]
    #[test_case(                  None, "127.0.0.1:7600",    Some("~/socket"), Some("6601"), MpdAddress::SocketPath("/home/u123/socket") ; "assume socket path with both MPD_HOST and MPD_PORT with tilde")]
    #[test_case( Some("/tmp/cli_sock"), "127.0.0.1:7600",                None,         None, MpdAddress::SocketPath("/tmp/cli_sock")     ; "prefer CLI with socket path over all")]
    #[test_case(                  None,  "/tmp/cfg_sock",                None,         None, MpdAddress::SocketPath("/tmp/cfg_sock")     ; "socket path from config")]
    fn resolves(cli: Option<&str>, config: &str, host: Option<&str>, port: Option<&str>, expected: MpdAddress) {
        let _guard = TEST_LOCK.lock().unwrap();

        ENV.clear();
        ENV.set("HOME".to_string(), "/home/u123".to_string());
        if let Some(host) = host {
            ENV.set("MPD_HOST".to_string(), host.to_string());
        };
        if let Some(port) = port {
            ENV.set("MPD_PORT".to_string(), port.to_string());
        }

        let result = MpdAddress::resolve(cli.map(|v| v.to_string()), config.to_string());

        assert_eq!(result, expected);
    }
}
