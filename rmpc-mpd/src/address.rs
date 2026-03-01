use rmpc_shared::{
    env::ENV,
    paths::utils::{env_var_expand, tilde_expand},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MpdAddress {
    IpAndPort(String),
    SocketPath(String),
    AbstractSocket(String),
}

#[derive(Default, Clone, Eq, PartialEq)]
pub struct MpdPassword(pub String);
impl std::fmt::Debug for MpdPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*****")
    }
}

impl From<&str> for MpdPassword {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<String> for MpdPassword {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl Default for MpdAddress {
    fn default() -> Self {
        Self::IpAndPort("127.0.0.1:6600".to_string())
    }
}

pub fn resolve(
    addr_from_cli: Option<String>,
    pw_from_cli: Option<String>,
    addr_from_config: String,
    pw_from_config: Option<String>,
) -> (MpdAddress, Option<MpdPassword>) {
    let (cli_addr, cli_pw) = resolve_cli(addr_from_cli, pw_from_cli);
    let (cfg_addr, cfg_pw) = resolve_config(addr_from_config, pw_from_config);
    let env = resolve_env();

    if let Some(cli_addr) = cli_addr {
        return (cli_addr, cli_pw);
    }

    if let Some(env) = env {
        return env;
    }

    (cfg_addr, cfg_pw)
}

fn resolve_config(addr: String, pw: Option<String>) -> (MpdAddress, Option<MpdPassword>) {
    let var_expanded = env_var_expand(&addr);
    let expanded = tilde_expand(&var_expanded);
    let addr = if expanded.starts_with('/') {
        MpdAddress::SocketPath(expanded.into_owned())
    } else if let Some(path) = expanded.strip_prefix('@') {
        MpdAddress::AbstractSocket(path.to_owned())
    } else {
        MpdAddress::IpAndPort(addr)
    };

    let pw: Option<MpdPassword> = pw.map(|pw| pw.into());

    (addr, pw)
}

fn resolve_cli(
    addr_from_cli: Option<String>,
    pw_from_cli: Option<String>,
) -> (Option<MpdAddress>, Option<MpdPassword>) {
    let addr = addr_from_cli.map(|addr| {
        let expanded = tilde_expand(&addr);
        if expanded.starts_with('/') {
            MpdAddress::SocketPath(expanded.into_owned())
        } else if let Some(path) = expanded.strip_prefix('@') {
            MpdAddress::AbstractSocket(path.to_owned())
        } else {
            MpdAddress::IpAndPort(addr)
        }
    });
    let pw: Option<MpdPassword> = pw_from_cli.map(|pw| pw.into());

    (addr, pw)
}

fn resolve_env() -> Option<(MpdAddress, Option<MpdPassword>)> {
    let mpd_host = ENV.var_os("MPD_HOST");
    let mpd_host = mpd_host.as_ref().and_then(|v| v.to_str());
    let mpd_port = ENV.var_os("MPD_PORT");
    let mpd_port = mpd_port.as_ref().and_then(|v| v.to_str());

    if let Some(host) = mpd_host {
        if !host.starts_with('@')
            && let Some((password, host)) = host.split_once('@')
        {
            let expanded = tilde_expand(host);
            if expanded.starts_with('/') {
                Some((
                    MpdAddress::SocketPath(expanded.into_owned()),
                    Some(password.to_string().into()),
                ))
            } else if let Some(path) = expanded.strip_prefix('@') {
                Some((
                    MpdAddress::AbstractSocket(path.to_owned()),
                    Some(password.to_string().into()),
                ))
            } else if let Some(port) = mpd_port {
                Some((
                    MpdAddress::IpAndPort(format!("{host}:{port}")),
                    Some(password.to_string().into()),
                ))
            } else {
                Some((
                    MpdAddress::IpAndPort(format!("{host}:6600")),
                    Some(password.to_string().into()),
                ))
            }
        } else {
            let expanded = tilde_expand(host);
            if expanded.starts_with('/') {
                Some((MpdAddress::SocketPath(expanded.into_owned()), None))
            } else if let Some(path) = expanded.strip_prefix('@') {
                Some((MpdAddress::AbstractSocket(path.to_owned()), None))
            } else if let Some(port) = mpd_port {
                Some((MpdAddress::IpAndPort(format!("{host}:{port}")), None))
            } else {
                Some((MpdAddress::IpAndPort(format!("{host}:6600")), None))
            }
        }
    } else {
        return None;
    }
}

#[cfg(test)]
#[rustfmt::skip]
#[allow(clippy::unwrap_used, clippy::too_many_arguments, clippy::needless_pass_by_value)]
mod tests {
    use std::sync::{LazyLock, Mutex};

    use rmpc_shared::env::ENV;
    use test_case::test_case;
    use crate::address::resolve;

    use super::{MpdAddress, MpdPassword};

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    //               CLI Arg              Cli Pass           Config addr     Config pw            MPD_HOST          MPD_PORT                    Expected                          Description
    #[test_case(Some("127.0.0.1:6600"),           None, "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("127.0.0.1:6600".to_string()),                       None ; "prefer CLI over all")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("192.168.0.1:6601".to_string()),                     None ; "prefer ENV over config")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                             None, Some("6601"), MpdAddress::IpAndPort("127.0.0.1:7600".to_string()),                       None ; "use config when only MPD_PORT")]
    #[test_case(Some("127.0.0.1:6600"),           None, "127.0.0.1:7600", None,                             None,         None, MpdAddress::IpAndPort("127.0.0.1:6600".to_string()),                       None ; "prefer CLI over config")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("/tmp/socket"),         None, MpdAddress::SocketPath("/tmp/socket".to_string()),                         None ; "assume socket path when only MPD_HOST")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                 Some("~/socket"),         None, MpdAddress::SocketPath("/home/u123/socket".to_string()),                   None ; "assume socket path when only MPD_HOST with tilde")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                     Some("@mpd"),         None, MpdAddress::AbstractSocket("mpd".to_string()),                             None ; "abstract socket in MPD_HOST")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("192.168.0.1"),         None, MpdAddress::IpAndPort("192.168.0.1:6600".to_string()),                     None ; "use 6600 as default port with only MPD_HOST")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("/tmp/socket"), Some("6601"), MpdAddress::SocketPath("/tmp/socket".to_string()),                         None ; "assume socket path with both MPD_HOST and MPD_PORT")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                 Some("~/socket"), Some("6601"), MpdAddress::SocketPath("/home/u123/socket".to_string()),                   None ; "assume socket path with both MPD_HOST and MPD_PORT with tilde")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,                     Some("@mpd"), Some("6601"), MpdAddress::AbstractSocket("mpd".to_string()),                             None ; "assume abstract socket with both MPD_HOST and MPD_PORT")]
    #[test_case( Some("/tmp/cli_sock"),           None, "127.0.0.1:7600", None,                             None,         None, MpdAddress::SocketPath("/tmp/cli_sock".to_string()),                       None ; "prefer CLI with socket path over all")]
    #[test_case(                  None,           None, "/tmp/cfg_sock",  None,                             None,         None, MpdAddress::SocketPath("/tmp/cfg_sock".to_string()),                       None ; "socket path from config")]
    #[test_case(                  None,           None, "~/cfg_sock",     None,                             None,         None, MpdAddress::SocketPath("/home/u123/cfg_sock".to_string()),                 None ; "socket path from config with tilde")]
    #[test_case(                  None,           None, "$HOME/cfg_sock", None,                             None,         None, MpdAddress::SocketPath("/home/u123/cfg_sock".to_string()),                 None ; "socket path from config with environment variable")]
    #[test_case(                  None,           None, "@mpd",           None,                             None,         None, MpdAddress::AbstractSocket("mpd".to_string()),                             None ; "abstract socket path from config")]
    #[test_case(Some("127.0.0.1:6600"), Some("secret"), "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("127.0.0.1:6600".to_string()),      Some("secret".into()) ; "CLI password")]
    #[test_case( Some("/tmp/cli_sock"), Some("secret"), "127.0.0.1:7600", None,              Some("192.168.0.1"), Some("6601"), MpdAddress::SocketPath("/tmp/cli_sock".to_string()),      Some("secret".into()) ; "CLI with socket path and password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,       Some("secret@192.168.0.1"), Some("6601"), MpdAddress::IpAndPort("192.168.0.1:6601".to_string()),    Some("secret".into()) ; "ENV password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,       Some("secret@/tmp/socket"), Some("6601"), MpdAddress::SocketPath("/tmp/socket".to_string()),        Some("secret".into()) ; "ENV with socket path and password")]
    #[test_case(                  None,           None, "/tmp/cfg_sock",  Some("secret"),                   None,         None, MpdAddress::SocketPath("/tmp/cfg_sock".to_string()),      Some("secret".into()) ; "socket path from config with password")]
    #[test_case(                  None,           None, "@mpd",           Some("secret"),                   None,         None, MpdAddress::AbstractSocket("mpd".to_string()),            Some("secret".into()) ; "abstract socket path from config with password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", None,              Some("secret@@mpd"),         None, MpdAddress::AbstractSocket("mpd".to_string()),            Some("secret".into()) ; "abstract socket path from ENV with password")]
    #[test_case(                  None,           None, "127.0.0.1:7600", Some("secret"),                   None,         None, MpdAddress::IpAndPort("127.0.0.1:7600".to_string()),      Some("secret".into()) ; "ip and port from config with password")]
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
        }
        if let Some(port) = port {
            ENV.set("MPD_PORT".to_string(), port.to_string());
        }

        let result = resolve(cli_addr.map(|v| v.to_string()), cli_pw.map(|v| v.to_string()), config_addr.to_string(), config_pw.map(|v| v.to_string()));

        assert_eq!(result.0, expected_addr);
        assert_eq!(result.1, expected_pw);
    }

    #[test]
    fn password_is_obfuscated() {
        let pw: MpdPassword = "verysecretpassword".to_string().into();

        assert_eq!(format!("{pw:?}"), "*****");
    }
}
