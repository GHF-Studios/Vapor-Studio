use super::{StudioApp, catalog_content_selector};
use super::operations::OperationResult;
use std::path::PathBuf;
use vapor_studio_core::OperationKind;

impl StudioApp {
    fn catalog_identity_is_conflicted(&self, selector: &str) -> bool {
        self.catalog.iter().any(|content| {
            catalog_content_selector(content) == selector && !content.is_unambiguous()
        })
    }

    pub(super) fn invalidate_workshop_review(&mut self) {
        self.workshop_preview = None;
        self.workshop_confirmation.clear();
    }

    pub(super) fn stage_workshop_content(&mut self, selector: String) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot stage {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_open_workspace("stage this Workshop item") {
            return;
        }
        self.invalidate_workshop_review();
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        self.spawn(OperationKind::StageWorkshop, move |_context| {
            OperationResult::WorkshopStage(vapor_studio_core::stage_workshop_preview(
                &executable,
                &workspace,
                &selector,
            ))
        });
        self.status = "Building and staging a Workshop review package…".to_owned();
    }

    pub(super) fn publish_reviewed_workshop_content(
        &mut self,
        selector: String,
        expected_review: vapor_studio_core::WorkshopPackagePreview,
    ) {
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot publish {selector}: local records for this exact version disagree. Review the catalog conflict first."
            );
            return;
        }
        if !self.require_open_workspace("publish this Workshop item") {
            return;
        }
        let account = self.workshop_account.trim().to_owned();
        if account.is_empty() {
            self.status =
                "Enter the Steam publisher account used by the local SteamCMD provider.".to_owned();
            return;
        }
        let expected_confirmation = format!("PUBLISH {selector}");
        if self.workshop_confirmation.trim() != expected_confirmation {
            self.status =
                format!("Type {expected_confirmation} exactly to authorize this Workshop upload.");
            return;
        }
        let workspace = PathBuf::from(self.workspace_root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let change_note = self.workshop_change_note.trim().to_owned();
        self.spawn(OperationKind::PublishWorkshop, move |_context| {
            OperationResult::WorkshopPublish(vapor_studio_core::publish_reviewed_workshop_content(
                &executable,
                &workspace,
                &selector,
                &expected_review,
                &account,
                (!change_note.is_empty()).then_some(change_note.as_str()),
            ))
        });
        self.status = "Uploading the reviewed Workshop artifact through Vapor Shell…".to_owned();
    }

    pub(super) fn download_workshop_item(&mut self) {
        let published_file_id = self.workshop_id.trim().to_owned();
        if published_file_id.is_empty() {
            self.status = "Enter a numeric Steam Workshop item ID.".to_owned();
            return;
        }
        if !published_file_id.bytes().all(|byte| byte.is_ascii_digit()) {
            self.status =
                "Workshop downloads currently require a numeric PublishedFileId.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        self.spawn(OperationKind::Acquire, move |_context| {
            OperationResult::WorkshopAcquisition(vapor_studio_core::download_workshop_item(
                &executable,
                &published_file_id,
            ))
        });
        self.status = "Downloading the public Workshop item into Vapor's cache…".to_owned();
    }

    pub(in crate::app) fn search_workshop(&mut self) {
        let query = self.workshop_search_text.trim().to_owned();
        let page = self.workshop_page;
        self.workshop_results.clear();
        self.workshop_results_query = None;
        self.workshop_result_total = 0;
        self.spawn(OperationKind::SearchWorkshop, move |_context| {
            OperationResult::WorkshopSearch(
                vapor_studio_core::search_workshop(
                    vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                    &query,
                    page,
                ),
                query,
            )
        });
        self.status = "Searching Steam Workshop…".to_owned();
    }

    pub(in crate::app) fn refresh_workshop_subscriptions(&mut self) {
        self.spawn(OperationKind::LoadWorkshopSubscriptions, move |_context| {
            OperationResult::WorkshopSubscriptions(vapor_studio_core::workshop_subscriptions(
                vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
            ))
        });
        self.status = "Loading Steam Workshop subscriptions…".to_owned();
    }

    pub(in crate::app) fn set_workshop_subscription(&mut self, id: String, subscribed: bool) {
        let parsed_id = match id.parse::<u64>() {
            Ok(id) if id != 0 => id,
            _ => {
                self.status = "Steam returned an invalid PublishedFileId.".to_owned();
                return;
            }
        };
        self.spawn(OperationKind::ChangeWorkshopSubscription, move |_context| {
            OperationResult::WorkshopSubscriptionChange(
                if subscribed {
                    vapor_studio_core::subscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                } else {
                    vapor_studio_core::unsubscribe_workshop_item(
                        vapor_sdk_core::root::types::ROOT_STEAM_APP_ID,
                        parsed_id,
                    )
                },
                id,
                subscribed,
            )
        });
        self.status = if subscribed {
            "Subscribing to Steam Workshop item…"
        } else {
            "Unsubscribing from Steam Workshop item…"
        }
        .to_owned();
    }

}
