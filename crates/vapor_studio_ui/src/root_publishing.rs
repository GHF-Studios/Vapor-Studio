use super::StudioApp;
use vapor_client_protocol::{ClientAction, PublishingAction};
use vapor_client_protocol::{RootDistributionIdentitySnapshot, RootPublishingSnapshot};

#[derive(Default)]
pub(super) struct RootPublishingState {
    pub(super) identity: RootDistributionIdentitySnapshot,
    pub(super) tab: PublishTab,
    pub(super) account: String,
    pub(super) description: String,
    pub(super) set_live_branch: String,
    pub(super) confirmation: String,
    pub(super) preview: Option<RootPublishingSnapshot>,
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
        self.submit_service_action(ClientAction::Publishing(
            PublishingAction::DiscardRootReview,
        ));
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
        self.root_publishing.preview = None;
        self.root_publishing.confirmation.clear();
        self.root_publishing.status = "Preparing a Steam upload review…".to_owned();
        self.start_service_operation(
            ClientAction::Publishing(PublishingAction::StageRoot {
                account,
                description,
                set_live,
            }),
            None,
            vapor_client_application::OperationKind::StageRootDistribution,
            vapor_client_application::OperationPhase::Staging,
            "Preparing the Steam upload review…",
        );
        self.status = self.root_publishing.status.clone();
    }

    pub(super) fn publish_steam_root(&mut self) {
        let Some(preview) = self.root_publishing.preview.as_ref() else {
            self.root_publishing.status =
                "Preview the Steam root package before uploading it.".to_owned();
            return;
        };
        if !preview.planned {
            self.root_publishing.status = "Stage a fresh release plan before uploading.".to_owned();
            return;
        }
        self.start_service_operation(
            ClientAction::Publishing(PublishingAction::PublishRoot {
                review_id: preview.review_id.clone(),
                confirmation: self.root_publishing.confirmation.clone(),
            }),
            None,
            vapor_client_application::OperationKind::PublishRootDistribution,
            vapor_client_application::OperationPhase::Publishing,
            "Uploading the reviewed Vapor release to Steam…",
        );
        self.root_publishing.status = "Uploading the reviewed Vapor release to Steam…".to_owned();
        self.status = self.root_publishing.status.clone();
    }
}

pub(super) fn root_publish_confirmation(app_id: u32, set_live: Option<&str>) -> String {
    match set_live {
        Some(branch) => format!("UPLOAD ROOT {app_id} AND SET LIVE {branch}"),
        None => format!("UPLOAD ROOT {app_id}"),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
