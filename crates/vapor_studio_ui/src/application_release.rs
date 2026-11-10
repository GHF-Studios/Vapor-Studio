use super::StudioApp;
use vapor_client_protocol::{ApplicationReleaseSnapshot, ClientAction, PublishingAction};

pub(super) struct ApplicationReleaseState {
    pub(super) tab: PublishTab,
    pub(super) account: String,
    pub(super) description: String,
    pub(super) branch: String,
    pub(super) confirmation: String,
    pub(super) preview: Option<ApplicationReleaseSnapshot>,
    pub(super) status: String,
    pub(super) linux: bool,
    pub(super) windows: bool,
}

impl Default for ApplicationReleaseState {
    fn default() -> Self {
        Self {
            tab: PublishTab::default(),
            account: String::new(),
            description: String::new(),
            branch: String::new(),
            confirmation: String::new(),
            preview: None,
            status: String::new(),
            linux: true,
            windows: true,
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum PublishTab {
    #[default]
    WorkshopContent,
    SteamProduct,
}

impl ApplicationReleaseState {
    pub(super) fn runtime_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();
        if self.linux {
            targets.push("x86_64-unknown-linux-gnu".to_owned());
        }
        if self.windows {
            targets.push("x86_64-pc-windows-gnullvm".to_owned());
        }
        targets
    }

    pub(super) fn confirmation_phrase(snapshot: &ApplicationReleaseSnapshot) -> String {
        format!("PUBLISH APPLICATION {} TO {}", snapshot.app_id, snapshot.branch)
    }
}

impl StudioApp {
    pub(super) fn invalidate_application_release_review(&mut self) {
        self.application_release.preview = None;
        self.application_release.confirmation.clear();
        self.submit_service_action(ClientAction::Publishing(
            PublishingAction::DiscardApplicationReview,
        ));
    }

    pub(super) fn stage_steam_product_release(&mut self) {
        let account = self.application_release.account.trim().to_owned();
        let runtime_targets = self.application_release.runtime_targets();
        if account.is_empty() || runtime_targets.is_empty() {
            self.application_release.status =
                "Enter the publisher account and select at least one target.".to_owned();
            return;
        }
        self.application_release.preview = None;
        self.application_release.confirmation.clear();
        self.application_release.status = "Staging the Loo-Cast content mount…".to_owned();
        self.start_service_operation(
            ClientAction::Publishing(PublishingAction::StageApplication {
                account,
                branch: non_empty(&self.application_release.branch),
                description: if self.application_release.description.trim().is_empty() {
                    "Loo-Cast content release".to_owned()
                } else {
                    self.application_release.description.trim().to_owned()
                },
                runtime_targets,
            }),
            None,
            vapor_client_application::OperationKind::StageApplicationRelease,
            vapor_client_application::OperationPhase::Staging,
            "Staging the Loo-Cast content mount…",
        );
        self.status = self.application_release.status.clone();
    }

    pub(super) fn publish_reviewed_steam_product(&mut self) {
        let Some(preview) = self.application_release.preview.as_ref() else {
            self.application_release.status = "Stage a release review before publishing.".to_owned();
            return;
        };
        if !preview.planned || !preview.payload_reviewed {
            self.application_release.status =
                "A fresh package review is required before upload.".to_owned();
            return;
        }
        self.start_service_operation(
            ClientAction::Publishing(PublishingAction::PublishApplication {
                review_id: preview.review_id.clone(),
                confirmation: self.application_release.confirmation.clone(),
            }),
            None,
            vapor_client_application::OperationKind::PublishApplicationRelease,
            vapor_client_application::OperationPhase::Publishing,
            "Uploading the reviewed Loo-Cast content release…",
        );
        self.application_release.status = "Uploading the reviewed Loo-Cast content release…".to_owned();
        self.status = self.application_release.status.clone();
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
