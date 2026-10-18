use crate::persistence::StudioPersistentState;
use vapor_studio_core::decode_profiles;
use vapor_studio_protocol::LegacyStorageSeed;

pub struct LegacyImport {
    pub state: StudioPersistentState,
    pub recovery_message: Option<String>,
}

pub fn import_legacy_storage(seed: Option<&LegacyStorageSeed>) -> LegacyImport {
    let Some(seed) = seed else {
        return LegacyImport {
            state: StudioPersistentState::default(),
            recovery_message: None,
        };
    };
    let (profiles, profile_recovery) = decode_profiles(seed.profiles.as_deref());
    let active_profile_id = seed
        .active_profile_id
        .as_deref()
        .filter(|id| profiles.profiles.iter().any(|profile| profile.id == *id))
        .map(str::to_owned)
        .or_else(|| {
            seed.active_profile_index
                .as_deref()
                .and_then(|index| index.parse::<usize>().ok())
                .and_then(|index| profiles.profiles.get(index))
                .map(|profile| profile.id.clone())
        })
        .unwrap_or_else(|| profiles.profiles[0].id.clone());
    let workspace_root = seed.workspace_root.clone().or_else(|| {
        profiles
            .profiles
            .iter()
            .find(|profile| profile.id == active_profile_id)
            .and_then(|profile| profile.workspace.as_ref())
            .map(|workspace| workspace.root.display().to_string())
    });
    let recovery_backup = seed.recovery_backup.clone().or_else(|| {
        profile_recovery
            .as_ref()
            .and_then(|_| seed.profiles.clone())
    });
    LegacyImport {
        state: StudioPersistentState {
            schema_version: 1,
            profile_revision: 0,
            profiles,
            active_profile_id,
            workspace_root,
            editor_drafts: seed.editor_drafts.clone(),
            operation_history: seed.operation_history.clone(),
            recovery_backup,
        },
        recovery_message: profile_recovery.map(|recovery| recovery.message),
    }
}
