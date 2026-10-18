use crate::legacy_import::import_legacy_storage;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use vapor_studio_protocol::{LegacyStorageSeed, ProfileDocument};

const STATE_SCHEMA_VERSION: u32 = 1;
const STATE_FILE_NAME: &str = "studio-state.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioPersistentState {
    pub schema_version: u32,
    #[serde(default)]
    pub profile_revision: u64,
    pub profiles: ProfileDocument,
    pub active_profile_id: String,
    pub workspace_root: Option<String>,
    pub editor_drafts: Option<String>,
    pub operation_history: Option<String>,
    pub recovery_backup: Option<String>,
}

impl Default for StudioPersistentState {
    fn default() -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            profile_revision: 0,
            profiles: ProfileDocument::default(),
            active_profile_id: "default".to_owned(),
            workspace_root: None,
            editor_drafts: None,
            operation_history: None,
            recovery_backup: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedStudioState {
    pub state: StudioPersistentState,
    pub legacy_imported: bool,
    pub recovery_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StudioStateStore {
    directory: PathBuf,
}

impl StudioStateStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn state_path(&self) -> PathBuf {
        self.directory.join(STATE_FILE_NAME)
    }

    fn temporary_path(&self) -> PathBuf {
        self.directory
            .join(format!(".{STATE_FILE_NAME}.tmp-{}", std::process::id()))
    }

    pub fn load_or_import(
        &self,
        legacy: Option<&LegacyStorageSeed>,
    ) -> Result<LoadedStudioState, String> {
        let path = self.state_path();
        match fs::read(&path) {
            Ok(bytes) => {
                let mut state: StudioPersistentState = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("could not read saved Studio state: {error}"))?;
                if state.schema_version != STATE_SCHEMA_VERSION {
                    return Err(format!(
                        "saved Studio state schema {} is unsupported",
                        state.schema_version
                    ));
                }
                clear_runtime_installation_state(&mut state);
                normalize_active_profile(&mut state);
                Ok(LoadedStudioState {
                    state,
                    legacy_imported: false,
                    recovery_message: None,
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let imported = import_legacy_storage(legacy);
                self.save(&imported.state)?;
                Ok(LoadedStudioState {
                    state: imported.state,
                    legacy_imported: legacy.is_some(),
                    recovery_message: imported.recovery_message,
                })
            }
            Err(error) => Err(format!("could not read saved Studio state: {error}")),
        }
    }

    pub fn save(&self, state: &StudioPersistentState) -> Result<(), String> {
        fs::create_dir_all(&self.directory)
            .map_err(|error| format!("could not create Studio state directory: {error}"))?;
        let mut durable = state.clone();
        clear_runtime_installation_state(&mut durable);
        let bytes = serde_json::to_vec_pretty(&durable)
            .map_err(|error| format!("could not encode Studio state: {error}"))?;
        let temporary = self.temporary_path();
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| format!("could not prepare Studio state replacement: {error}"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("could not write Studio state replacement: {error}"))?;
            fs::rename(&temporary, self.state_path())
                .map_err(|error| format!("could not replace saved Studio state: {error}"))?;
            sync_directory(&self.directory)
                .map_err(|error| format!("could not sync Studio state directory: {error}"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn clear_runtime_installation_state(state: &mut StudioPersistentState) {
    for profile in &mut state.profiles.profiles {
        profile.installation = vapor_studio_protocol::InstallationState::Unknown;
    }
}

fn normalize_active_profile(state: &mut StudioPersistentState) {
    if !state
        .profiles
        .profiles
        .iter()
        .any(|profile| profile.id == state.active_profile_id)
    {
        state.active_profile_id = state
            .profiles
            .profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_else(|| "default".to_owned());
    }
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> std::io::Result<()> {
    fs::File::open(directory)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_directory(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "vapor-studio-{test_name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create temporary state directory");
        directory
    }

    #[test]
    fn imports_legacy_egui_values_once_and_round_trips_after_restart() {
        let directory = temp_directory("legacy-import");
        let store = StudioStateStore::new(directory.clone());
        let seed = LegacyStorageSeed {
            profiles: Some(
                r#"{"schema_version":1,"profiles":[{"id":"work","name":"Work"}]}"#.into(),
            ),
            active_profile_id: Some("work".into()),
            active_profile_index: Some("0".into()),
            workspace_root: Some("/projects/work".into()),
            editor_drafts: Some(r#"{"files":["draft"]}"#.into()),
            operation_history: Some(r#"[{"action":"refresh"}]"#.into()),
            recovery_backup: Some("older-backup".into()),
        };

        let imported = store
            .load_or_import(Some(&seed))
            .expect("import legacy state");
        assert!(imported.legacy_imported);
        assert_eq!(imported.state.active_profile_id, "work");
        assert_eq!(
            imported.state.workspace_root.as_deref(),
            Some("/projects/work")
        );
        assert_eq!(
            imported.state.editor_drafts.as_deref(),
            Some(r#"{"files":["draft"]}"#)
        );
        assert_eq!(
            imported.state.operation_history.as_deref(),
            Some(r#"[{"action":"refresh"}]"#)
        );

        let replacement_seed = LegacyStorageSeed {
            profiles: None,
            active_profile_id: Some("wrong".into()),
            active_profile_index: None,
            workspace_root: None,
            editor_drafts: None,
            operation_history: None,
            recovery_backup: None,
        };
        let restarted = StudioStateStore::new(directory.clone())
            .load_or_import(Some(&replacement_seed))
            .expect("load persisted state after restart");
        assert!(!restarted.legacy_imported);
        assert_eq!(restarted.state.active_profile_id, "work");
        assert_eq!(restarted.state.profiles.profiles[0].name, "Work");
        assert_eq!(
            restarted.state.recovery_backup.as_deref(),
            Some("older-backup")
        );

        fs::remove_dir_all(directory).expect("remove temporary state directory");
    }

    #[test]
    fn malformed_legacy_profiles_recover_to_default_and_preserve_the_original() {
        let directory = temp_directory("malformed-legacy");
        let seed = LegacyStorageSeed {
            profiles: Some("{ definitely not profile json".into()),
            active_profile_id: Some("missing".into()),
            active_profile_index: Some("999".into()),
            workspace_root: None,
            editor_drafts: None,
            operation_history: None,
            recovery_backup: None,
        };

        let imported = StudioStateStore::new(directory.clone())
            .load_or_import(Some(&seed))
            .expect("malformed legacy data recovers safely");
        assert!(imported.legacy_imported);
        assert_eq!(imported.state.profiles.profiles.len(), 1);
        assert_eq!(imported.state.profiles.profiles[0].id, "default");
        assert_eq!(imported.state.active_profile_id, "default");
        assert_eq!(
            imported.state.recovery_backup.as_deref(),
            Some("{ definitely not profile json")
        );
        assert!(imported.recovery_message.is_some());

        fs::remove_dir_all(directory).expect("remove temporary state directory");
    }

    #[test]
    fn failed_atomic_replacement_keeps_the_previous_state_intact() {
        let directory = temp_directory("failed-save");
        let store = StudioStateStore::new(directory.clone());
        let initial = store
            .load_or_import(None)
            .expect("write initial state")
            .state;
        let original_bytes = fs::read(store.state_path()).expect("read initial state file");
        fs::create_dir(store.temporary_path()).expect("block temporary replacement path");

        let mut changed = initial;
        changed.active_profile_id = "changed".into();
        assert!(store.save(&changed).is_err());
        assert_eq!(
            fs::read(store.state_path()).expect("read preserved state"),
            original_bytes
        );

        fs::remove_dir_all(directory).expect("remove temporary state directory");
    }

    #[test]
    fn runtime_installation_state_is_cleared_from_the_durable_file() {
        let directory = temp_directory("runtime-installation");
        let store = StudioStateStore::new(directory.clone());
        let mut state = StudioPersistentState::default();
        state.profiles.profiles[0].installation =
            vapor_studio_protocol::InstallationState::NotInstalled;

        store.save(&state).expect("save durable profile state");
        let bytes = fs::read(store.state_path()).expect("read durable state file");
        let saved: StudioPersistentState =
            serde_json::from_slice(&bytes).expect("decode durable state");
        assert_eq!(
            saved.profiles.profiles[0].installation,
            vapor_studio_protocol::InstallationState::Unknown
        );

        let loaded = store
            .load_or_import(None)
            .expect("load durable state after restart");
        assert_eq!(
            loaded.state.profiles.profiles[0].installation,
            vapor_studio_protocol::InstallationState::Unknown
        );
        fs::remove_dir_all(directory).expect("remove temporary state directory");
    }
}
