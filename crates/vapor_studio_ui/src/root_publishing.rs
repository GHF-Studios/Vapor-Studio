use super::StudioApp;
use super::operation_protocol::OperationResult;
use std::path::PathBuf;
use vapor_sdk_core::root::types::{ROOT_STEAM_APP_ID, ROOT_STEAM_DEPOT_ID, RootPublishRequest};
use vapor_studio_core::OperationKind;

#[derive(Default)]
pub(super) struct RootPublishingState {
    pub(super) tab: PublishTab,
    pub(super) account: String,
    pub(super) description: String,
    pub(super) set_live_branch: String,
    pub(super) confirmation: String,
    pub(super) preview: Option<vapor_sdk_core::root::types::RootPublishReport>,
    pub(super) status: String,
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum PublishTab {
    #[default]
    WorkshopContent,
    VaporApp,
}

impl StudioApp {
    pub(super) fn invalidate_root_publish_review(&mut self) {
        self.root_publishing.preview = None;
        self.root_publishing.confirmation.clear();
    }

    pub(super) fn preview_steam_package(&mut self) {
        let account = self.root_publishing.account.trim().to_owned();
        if account.is_empty() {
            self.root_publishing.status =
                "Enter the Steamworks publisher account before previewing the upload.".to_owned();
            return;
        }
        let description = if self.root_publishing.description.trim().is_empty() {
            "Vapor root package".to_owned()
        } else {
            self.root_publishing.description.trim().to_owned()
        };
        let set_live = non_empty(&self.root_publishing.set_live_branch);
        let request = RootPublishRequest {
            plan: true,
            app_id: ROOT_STEAM_APP_ID,
            depot_id: ROOT_STEAM_DEPOT_ID,
            description,
            account,
            steamcmd: PathBuf::from("steamcmd"),
            set_live,
        };
        self.root_publishing.preview = None;
        self.root_publishing.confirmation.clear();
        self.root_publishing.status =
            "Preparing a read-only SteamPipe package and upload plan…".to_owned();
        self.spawn(OperationKind::StageRootDistribution, move |_context| {
            OperationResult::RootPublish(
                vapor_sdk_core::root::publish::root_publish(&request)
                    .map_err(|error| error.to_string()),
            )
        });
        self.status = self.root_publishing.status.clone();
    }

    pub(super) fn publish_steam_root(&mut self) {
        let Some(preview) = self.root_publishing.preview.as_ref() else {
            self.root_publishing.status =
                "Preview the Steam root package before uploading it.".to_owned();
            return;
        };
        let account = self.root_publishing.account.trim().to_owned();
        let description = if self.root_publishing.description.trim().is_empty() {
            "Vapor root package".to_owned()
        } else {
            self.root_publishing.description.trim().to_owned()
        };
        let set_live = non_empty(&self.root_publishing.set_live_branch);
        let expected_confirmation = root_publish_confirmation(set_live.as_deref());
        if !preview.planned
            || preview.account != account
            || preview.package.description != description
            || preview.set_live != set_live
        {
            self.invalidate_root_publish_review();
            self.root_publishing.status =
                "The Steam package settings changed. Review a fresh upload plan first.".to_owned();
            return;
        }
        if self.root_publishing.confirmation.trim() != expected_confirmation {
            self.root_publishing.status =
                format!("Type {expected_confirmation} exactly to authorize this SteamPipe upload.");
            return;
        }

        let request = RootPublishRequest {
            plan: false,
            app_id: ROOT_STEAM_APP_ID,
            depot_id: ROOT_STEAM_DEPOT_ID,
            description,
            account,
            steamcmd: preview.steamcmd.clone(),
            set_live,
        };
        self.spawn(OperationKind::PublishRootDistribution, move |_context| {
            OperationResult::RootPublish(
                vapor_sdk_core::root::publish::root_publish(&request)
                    .map_err(|error| error.to_string()),
            )
        });
        self.root_publishing.status =
            "Uploading the reviewed Vapor root build through SteamCMD…".to_owned();
        self.status = self.root_publishing.status.clone();
    }
}

pub(super) fn root_publish_confirmation(set_live: Option<&str>) -> String {
    match set_live {
        Some(branch) => format!("UPLOAD ROOT {ROOT_STEAM_APP_ID} AND SET LIVE {branch}"),
        None => format!("UPLOAD ROOT {ROOT_STEAM_APP_ID}"),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
