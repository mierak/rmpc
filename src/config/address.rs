use crate::shared::env::ENV;

use super::utils::tilde_expand;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MpdAddress<'a> {
    IpAndPort(&'a str),
    SocketPath(&'a str),
}

#[derive(Default, Clone, Copy, Eq, PartialEq)]
pub struct MpdPassword<'a>(pub &'a str);
impl std::fmt::Debug for MpdPassword<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*****")
    }
}

impl From<&str> for MpdPassword<'static> {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<String> for MpdPassword<'static> {
    fn from(s: String) -> Self {
        Self(super::Leak::leak(s))
    }
}

impl Default for MpdAddress<'_> {
    fn default() -> Self {
        Self::IpAndPort("127.0.0.1:6600")
    }
}

impl MpdAddress<'static> {
    pub fn resolve(
        addr_from_cli: Option<String>,
        pw_from_cli: Option<String>,
        addr_from_config: String,
        pw_from_config: Option<String>,
    ) -> (MpdAddress<'static>, Option<MpdPassword<'static>>) {
        let (cli_addr, cli_pw) = Self::resolve_cli(addr_from_cli, pw_from_cli);
        let (cfg_addr, cfg_pw) = Self::resolve_config(addr_from_config, pw_from_config);
        let env = Self::resolve_env();

        if let Some(cli_addr) = cli_addr {
            return (cli_addr, cli_pw);
        }

        if let Some(env) = env {
            return env;
        }

        (cfg_addr, cfg_pw)
    }

    fn resolve_config(addr: String, pw: Option<String>) -> (MpdAddress<'static>, Option<MpdPassword<'static>>) {
        let expanded = tilde_expand(&addr);
        let addr = if expanded.starts_with('/') {
            MpdAddress::SocketPath(expanded.into_owned().leak())
        } else {
            MpdAddress::IpAndPort(addr.leak())
        };

        let pw: Option<MpdPassword<'_>> = pw.map(|pw| pw.into());

        (addr, pw)
    }

    fn resolve_cli(
        addr_from_cli: Option<String>,
        pw_from_cli: Option<String>,
    ) -> (Option<MpdAddress<'static>>, Option<MpdPassword<'static>>) {
        let addr = addr_from_cli.map(|addr| {
            let expanded = tilde_expand(&addr);
            if expanded.starts_with('/') {
                MpdAddress::SocketPath(expanded.into_owned().leak())
            } else {
                MpdAddress::IpAndPort(addr.leak())
            }
        });
        let pw: Option<MpdPassword<'_>> = pw_from_cli.map(|pw| pw.into());

        (addr, pw)
    }

    fn resolve_env() -> Option<(MpdAddress<'static>, Option<MpdPassword<'static>>)> {
        let mpd_host = ENV.var_os("MPD_HOST");
        let mpd_host = mpd_host.as_ref().and_then(|v| v.to_str());
        let mpd_port = ENV.var_os("MPD_PORT");
        let mpd_port = mpd_port.as_ref().and_then(|v| v.to_str());

        if let Some(host) = mpd_host {
            if let Some((password, host)) = host.split_once('@') {
                let expanded = tilde_expand(host);
                if expanded.starts_with('/') {
                    Some((
                        MpdAddress::SocketPath(expanded.into_owned().leak()),
                        Some(password.to_string().into()),
                    ))
                } else if let Some(port) = mpd_port {
                    Some((
                        MpdAddress::IpAndPort(format!("{host}:{port}").leak()),
                        Some(password.to_string().into()),
                    ))
                } else {
                    Some((
                        MpdAddress::IpAndPort(format!("{host}:6600").leak()),
                        Some(password.to_string().into()),
                    ))
                }
            } else {
                let expanded = tilde_expand(host);
                if expanded.starts_with('/') {
                    Some((MpdAddress::SocketPath(expanded.into_owned().leak()), None))
                } else if let Some(port) = mpd_port {
                    Some((MpdAddress::IpAndPort(format!("{host}:{port}").leak()), None))
                } else {
                    Some((MpdAddress::IpAndPort(format!("{host}:6600").leak()), None))
                }
            }
        } else {
            return None;
        }
    }
}

#[cfg(test)]
#[rustfmt::skip]
#[allow(clippy::unwrap_used, clippy::too_many_arguments)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use test_case::test_case;
    use crate::shared::env::ENV;
    use super::{MpdAddress, MpdPassword};

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    //               CLI Arg              Cli Pass           Config addr     Config pw            MPD_HOST          MPD_PORT                    Expected                          Description
    #[test_case(Some("127.0.0.1:6600"),           None, "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("127.0.0.1:6600"),                       None ; "prefer CLI over all")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("192.168.0.1:6601"),                     None ; "prefer ENV over config")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                             None, Some("6601"), MpdAddress::IpAndPort("127.0.0.1:7600"),                       None ; "use config when only MPD_PORT")]
    #[test_case(Some("127.0.0.1:6600"),           None, "127.0.0.1:7600", None,                             None,         None, MpdAddress::IpAndPort("127.0.0.1:6600"),                       None ; "prefer CLI over config")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("/tmp/socket"),         None, MpdAddress::SocketPath("/tmp/socket"),                         None ; "assume socket path when only MPD_HOST")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                 Some("~/socket"),         None, MpdAddress::SocketPath("/home/u123/socket"),                   None ; "assume socket path when only MPD_HOST with tilde")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("192.168.0.1"),         None, MpdAddress::IpAndPort("192.168.0.1:6600"),                     None ; "use 6600 as default port with only MPD_HOST")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("/tmp/socket"), Some("6601"), MpdAddress::SocketPath("/tmp/socket"),                         None ; "assume socket path with both MPD_HOST and MPD_PORT")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                 Some("~/socket"), Some("6601"), MpdAddress::SocketPath("/home/u123/socket"),                   None ; "assume socket path with both MPD_HOST and MPD_PORT with tilde")]
    #[test_case( Some("/tmp/cli_sock"),           None, "127.0.0.1:7600", None,                             None,         None, MpdAddress::SocketPath("/tmp/cli_sock"),                       None ; "prefer CLI with socket path over all")]
    #[test_case(                  None,           None, "/tmp/cfg_sock",  None,                             None,         None, MpdAddress::SocketPath("/tmp/cfg_sock"),                       None ; "socket path from config")]
    #[test_case(Some("127.0.0.1:6600"), Some("secret"), "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("127.0.0.1:6600"),      Some("secret".into()) ; "CLI password")]
    #[test_case( Some("/tmp/cli_sock"), Some("secret"), "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::SocketPath("/tmp/cli_sock"),      Some("secret".into()) ; "CLI with socket path and password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,       Some("secret@192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("192.168.0.1:6601"),    Some("secret".into()) ; "ENV password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,       Some("secret@/tmp/socket"), Some("6601"), MpdAddress::SocketPath("/tmp/socket"),        Some("secret".into()) ; "ENV with socket path and password")]
    #[test_case(                  None,           None, "/tmp/cfg_sock",  Some("secret"),                   None,         None, MpdAddress::SocketPath("/tmp/cfg_sock"),      Some("secret".into()) ; "socket path from config with password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", Some("secret"),                   None,         None, MpdAddress::IpAndPort("127.0.0.1:7600"),      Some("secret".into()) ; "ip and port from config with password")]
    fn resolves(
        cli_addr: Option<&str>,
        cli_pw: Option<&str>,
        config_addr: &str,
        config_pw: Option<&str>,
        host: Option<&str>,
        port: Option<&str>,
        expected_addr: MpdAddress,
        expected_pw: Option<MpdPassword>
    ) {
        let _guard = TEST_LOCK.lock().unwrap();

        ENV.clear();
        ENV.set("HOME".to_string(), "/home/u123".to_string());
        if let Some(host) = host {
            ENV.set("MPD_HOST".to_string(), host.to_string());
        };
        if let Some(port) = port {
            ENV.set("MPD_PORT".to_string(), port.to_string());
        }

        let result = MpdAddress::resolve(cli_addr.map(|v| v.to_string()), cli_pw.map(|v| v.to_string()), config_addr.to_string(), config_pw.map(|v| v.to_string()));

        assert_eq!(result.0, expected_addr);
        assert_eq!(result.1, expected_pw);
    }

    #[test]
    fn password_is_obfuscated() {
        let pw: MpdPassword<'static> = "verysecretpassword".to_string().into();

        assert_eq!(format!("{pw:?}"), "*****");
    }
}
