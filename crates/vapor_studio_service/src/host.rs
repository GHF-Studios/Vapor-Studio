use std::{collections::HashMap, ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    Linux,
    Windows,
    MacOS,
    Other,
}

impl HostPlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioHostArguments {
    pub application_executable: PathBuf,
    pub workspace_root: Option<PathBuf>,
    pub state_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioHostContext {
    pub application_executable: PathBuf,
    pub app_root: Option<PathBuf>,
    pub workspace_root: Option<PathBuf>,
    pub state_directory: PathBuf,
    pub platform: HostPlatform,
    pub steam_client_available: bool,
}

impl StudioHostContext {
    pub fn resolve(
        args: StudioHostArguments,
        inherited_environment: &HashMap<OsString, OsString>,
        platform: HostPlatform,
    ) -> Result<Self, String> {
        let workspace_root = args.workspace_root.or_else(|| {
            env_value(inherited_environment, "VAPOR_STUDIO_WORKSPACE").map(PathBuf::from)
        });
        let state_directory = args.state_directory
            .or_else(|| env_value(inherited_environment, "VAPOR_STUDIO_STATE_DIR").map(PathBuf::from))
            .or_else(|| default_state_directory(platform, inherited_environment))
            .ok_or_else(|| match platform {
                HostPlatform::Windows => "Studio user data path is unavailable; set APPDATA or VAPOR_STUDIO_STATE_DIR".to_owned(),
                HostPlatform::MacOS => "Studio user data path is unavailable; set HOME or VAPOR_STUDIO_STATE_DIR".to_owned(),
                HostPlatform::Linux => "Studio user data path is unavailable; set XDG_DATA_HOME, HOME, or VAPOR_STUDIO_STATE_DIR".to_owned(),
                HostPlatform::Other => "Studio user data path is unavailable; set VAPOR_STUDIO_STATE_DIR".to_owned(),
            })?;
        let app_root = args
            .application_executable
            .parent()
            .and_then(|directory| directory.parent())
            .map(PathBuf::from);
        let steam_client_available = [
            "SteamAppId",
            "SteamGameId",
            "STEAM_COMPAT_CLIENT_INSTALL_PATH",
            "SteamPath",
        ]
        .iter()
        .any(|name| env_value(inherited_environment, name).is_some());
        Ok(Self {
            application_executable: args.application_executable,
            app_root,
            workspace_root,
            state_directory,
            platform,
            steam_client_available,
        })
    }
}

fn default_state_directory(
    platform: HostPlatform,
    environment: &HashMap<OsString, OsString>,
) -> Option<PathBuf> {
    match platform {
        HostPlatform::Windows => env_value(environment, "APPDATA")
            .or_else(|| env_value(environment, "LOCALAPPDATA"))
            .map(|root| PathBuf::from(root).join("Vapor").join("Studio")),
        HostPlatform::MacOS => env_value(environment, "HOME")
            .map(|root| PathBuf::from(root).join("Library/Application Support/Vapor/Studio")),
        HostPlatform::Linux => env_value(environment, "XDG_DATA_HOME")
            .map(|root| PathBuf::from(root).join("vapor/studio"))
            .or_else(|| {
                env_value(environment, "HOME")
                    .map(|root| PathBuf::from(root).join(".local/share/vapor/studio"))
            }),
        HostPlatform::Other => None,
    }
}

fn env_value<'a>(environment: &'a HashMap<OsString, OsString>, key: &str) -> Option<&'a OsString> {
    environment.get(OsString::from(key).as_os_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, ffi::OsString, path::PathBuf};

    fn env(entries: &[(&str, &str)]) -> HashMap<OsString, OsString> {
        entries
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect()
    }

    #[test]
    fn explicit_arguments_override_environment() {
        let context = StudioHostContext::resolve(
            StudioHostArguments {
                application_executable: PathBuf::from("/install/bin/vapor-gui"),
                workspace_root: Some(PathBuf::from("/explicit/workspace")),
                state_directory: Some(PathBuf::from("/explicit/state")),
            },
            &env(&[
                ("VAPOR_STUDIO_WORKSPACE", "/environment/workspace"),
                ("VAPOR_STUDIO_STATE_DIR", "/environment/state"),
                ("XDG_DATA_HOME", "/environment/data"),
            ]),
            HostPlatform::Linux,
        )
        .expect("host context resolves");

        assert_eq!(
            context.workspace_root,
            Some(PathBuf::from("/explicit/workspace"))
        );
        assert_eq!(context.state_directory, PathBuf::from("/explicit/state"));
    }

    #[test]
    fn missing_optional_environment_keeps_workspace_unset_and_uses_user_data_default() {
        let context = StudioHostContext::resolve(
            StudioHostArguments {
                application_executable: PathBuf::from("/install/bin/vapor-gui"),
                workspace_root: None,
                state_directory: None,
            },
            &env(&[("HOME", "/home/player")]),
            HostPlatform::Linux,
        )
        .expect("host context resolves without optional values");

        assert_eq!(context.workspace_root, None);
        assert_eq!(
            context.state_directory,
            PathBuf::from("/home/player/.local/share/vapor/studio")
        );
    }

    #[test]
    fn secrets_in_inherited_environment_are_never_retained_or_formatted() {
        let context = StudioHostContext::resolve(
            StudioHostArguments {
                application_executable: PathBuf::from("/install/bin/vapor-gui"),
                workspace_root: None,
                state_directory: None,
            },
            &env(&[
                ("HOME", "/home/player"),
                ("STEAM_PASSWORD", "never-print-this-secret"),
                ("VAPOR_STEAM_TOKEN", "also-secret"),
            ]),
            HostPlatform::Linux,
        )
        .expect("host context resolves");

        let diagnostic = format!("{context:?}");
        assert!(!diagnostic.contains("never-print-this-secret"));
        assert!(!diagnostic.contains("also-secret"));
    }
}
