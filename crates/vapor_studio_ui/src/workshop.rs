use super::{StudioApp, catalog_content_selector};
use super::operation_protocol::OperationResult;
use std::path::PathBuf;
use vapor_studio_core::OperationKind;

pub(super) struct WorkshopState {
    pub(super) preview: Option<vapor_studio_core::WorkshopPackagePreview>,
    pub(super) published_file_id: String,
    pub(super) publisher_account: String,
    pub(super) change_note: String,
    pub(super) confirmation: String,
    pub(super) selected_items: std::collections::BTreeSet<String>,
    pub(super) pending_acquisitions: Vec<vapor_studio_core::WorkshopAcquisition>,
    pub(super) search_query: String,
    pub(super) results: Vec<vapor_studio_core::WorkshopItem>,
    pub(super) results_query: Option<String>,
    pub(super) subscriptions: std::collections::BTreeSet<String>,
    pub(super) subscription_details:
        std::collections::BTreeMap<String, vapor_studio_core::WorkshopSubscription>,
    pub(super) total_results: u32,
    pub(super) page: u32,
}

impl Default for WorkshopState {
    fn default() -> Self {
        Self {
            preview: None,
            published_file_id: String::new(),
            publisher_account: String::new(),
            change_note: String::new(),
            confirmation: String::new(),
            selected_items: Default::default(),
            pending_acquisitions: Vec::new(),
            search_query: String::new(),
            results: Vec::new(),
            results_query: None,
            subscriptions: Default::default(),
            subscription_details: Default::default(),
            total_results: 0,
            page: 1,
        }
    }
}

impl StudioApp {
    fn catalog_identity_is_conflicted(&self, selector: &str) -> bool {
        self.catalog.items.iter().any(|content| {
            catalog_content_selector(content) == selector && !content.is_unambiguous()
        })
    }

    pub(super) fn invalidate_workshop_review(&mut self) {
        self.workshop.preview = None;
        self.workshop.confirmation.clear();
    }

    pub(super) fn apply_workshop_stage_error(&mut self, error: String) {
        self.invalidate_workshop_review();
        self.status = error;
    }

    pub(super) fn apply_workshop_error(&mut self, error: String) {
        self.status = error;
    }

    pub(super) fn apply_workshop_acquisition(
        &mut self,
        acquisitions: Vec<vapor_studio_core::WorkshopAcquisition>,
    ) {
        self.status = format!(
            "Imported {} Workshop item(s) into Vapor's cache.",
            acquisitions.len()
        );
        self.workshop.pending_acquisitions = acquisitions;
        self.workshop.selected_items.clear();
    }

    pub(super) fn apply_workshop_publication(
        &mut self,
        publication: vapor_studio_core::WorkshopPublication,
    ) {
        self.status = format!(
            "{} Workshop item {}{}; receipt saved at {}.",
            if publication.created {
                "Created"
            } else {
                "Updated"
            },
            publication.id,
            publication
                .published_file_id
                .as_ref()
                .map(|id| format!(" · PublishedFileId {id}"))
                .unwrap_or_default(),
            publication.receipt.display(),
        );
        self.invalidate_workshop_review();
    }

    pub(super) fn apply_workshop_stage(
        &mut self,
        preview: vapor_studio_core::WorkshopPackagePreview,
    ) {
        self.status = format!(
            "Staged {} · {} files · {} bytes.",
            preview.id, preview.files, preview.bytes
        );
        self.workshop.preview = Some(preview);
    }

    pub(super) fn apply_workshop_search(
        &mut self,
        page: vapor_studio_core::WorkshopPage,
        query: String,
    ) {
        self.workshop.page = page.page;
        self.workshop.total_results = page.total_results;
        self.workshop.results = page.items;
        self.workshop.results_query = Some(query);
        self.status = format!(
            "Steam Workshop returned {} items on page {} of {}.",
            self.workshop.results.len(),
            self.workshop.page,
            self.workshop.total_results
        );
    }

    pub(super) fn apply_workshop_subscriptions(
        &mut self,
        subscriptions: Vec<vapor_studio_core::WorkshopSubscription>,
    ) {
        self.workshop.subscriptions = subscriptions
            .iter()
            .map(|item| item.published_file_id.clone())
            .collect();
        self.workshop.subscription_details = subscriptions
            .into_iter()
            .map(|item| (item.published_file_id.clone(), item))
            .collect();
        self.status = format!(
            "Loaded {} Steam Workshop subscriptions.",
            self.workshop.subscriptions.len()
        );
    }

    pub(super) fn apply_workshop_subscription_change(
        &mut self,
        id: String,
        subscribed: bool,
    ) {
        if subscribed {
            self.workshop.subscriptions.insert(id.clone());
        } else {
            self.workshop.subscriptions.remove(&id);
            self.workshop.subscription_details.remove(&id);
        }
        self.status = format!(
            "{} Steam Workshop item {id}.",
            if subscribed {
                "Subscribed to"
            } else {
                "Unsubscribed from"
            }
        );
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
        let workspace = PathBuf::from(self.workspace.root.trim());
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
        let account = self.workshop.publisher_account.trim().to_owned();
        if account.is_empty() {
            self.status =
                "Enter the Steam publisher account used by the local SteamCMD provider.".to_owned();
            return;
        }
        let expected_confirmation = format!("PUBLISH {selector}");
        if self.workshop.confirmation.trim() != expected_confirmation {
            self.status =
                format!("Type {expected_confirmation} exactly to authorize this Workshop upload.");
            return;
        }
        let workspace = PathBuf::from(self.workspace.root.trim());
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        };
        let change_note = self.workshop.change_note.trim().to_owned();
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
        let published_file_id = self.workshop.published_file_id.trim().to_owned();
        if published_file_id.is_empty() {
            self.status = "Enter a numeric Steam Workshop item ID.".to_owned();
            return;
        }
        if !published_file_id.bytes().all(|byte| byte.is_ascii_digit()) {
            self.status =
                "Workshop downloads currently require a numeric PublishedFileId.".to_owned();
            return;
        }
        self.download_workshop_items(vec![published_file_id]);
    }

    pub(super) fn download_selected_workshop_items(&mut self) {
        let ids = self.workshop.selected_items.iter().cloned().collect::<Vec<_>>();
        self.download_workshop_items(ids);
    }

    fn download_workshop_items(&mut self, published_file_ids: Vec<String>) {
        if published_file_ids.is_empty() {
            self.status = "Select at least one Workshop item to import.".to_owned();
            return;
        }
        if !published_file_ids
            .iter()
            .all(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
        {
            self.status = "Workshop downloads require numeric PublishedFileIds.".to_owned();
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("Could not locate the Studio executable: {error}");
                return;
            }
        };
        let count = published_file_ids.len();
        self.spawn(OperationKind::Acquire, move |_context| {
            OperationResult::WorkshopAcquisition(vapor_studio_core::download_workshop_items(
                &executable,
                &published_file_ids,
            ))
        });
        self.status = format!(
            "Importing {count} public Workshop item(s) into Vapor's cache…"
        );
    }

    pub(in crate::app) fn search_workshop(&mut self) {
        let query = self.workshop.search_query.trim().to_owned();
        let page = self.workshop.page;
        self.workshop.results.clear();
        self.workshop.results_query = None;
        self.workshop.total_results = 0;
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
