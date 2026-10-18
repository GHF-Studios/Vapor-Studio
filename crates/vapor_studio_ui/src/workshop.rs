use super::{StudioApp, catalog_content_selector};
use crate::navigation::CatalogSourceFilter;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vapor_client_application::OperationKind;

const WORKSHOP_PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(2);
const WORKSHOP_PROGRESS_POLL_MAX_INTERVAL: Duration = Duration::from_secs(30);
const MAX_RECENT_WORKSHOP_ACQUISITIONS: usize = 50;

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum WorkshopTab {
    #[default]
    Browse,
    Subscriptions,
    Imports,
}

pub(super) struct WorkshopState {
    pub(super) tab: WorkshopTab,
    pub(super) preview: Option<vapor_client_application::WorkshopPackagePreview>,
    pub(super) published_file_id: String,
    pub(super) publisher_account: String,
    pub(super) change_note: String,
    pub(super) confirmation: String,
    pub(super) selected_items: std::collections::BTreeSet<String>,
    pub(super) recent_acquisitions: Vec<vapor_client_application::WorkshopAcquisition>,
    pub(super) pending_acquisition_ids: std::collections::BTreeSet<String>,
    pub(super) service_review_id: Option<String>,
    pub(super) omitted_review_file_paths: usize,
    pub(super) search_query: String,
    pub(super) pending_search: Option<(String, u32)>,
    pub(super) service_search_revision: u64,
    pub(super) pending_service_search_revision: Option<u64>,
    pub(super) service_acquisition_revision: u64,
    pub(super) results: Vec<vapor_client_application::WorkshopItem>,
    pub(super) focused_result: Option<String>,
    pub(super) results_query: Option<String>,
    pub(super) subscriptions: std::collections::BTreeSet<String>,
    pub(super) subscriptions_loaded: bool,
    pub(super) subscriptions_stale: bool,
    pub(super) service_subscriptions_revision: Option<u64>,
    pub(super) subscription_search_query: String,
    pub(super) subscription_details:
        std::collections::BTreeMap<String, vapor_client_application::WorkshopSubscription>,
    pub(super) subscription_poll_interval: Duration,
    pub(super) next_subscription_poll: Option<Instant>,
    pub(super) total_results: u32,
    pub(super) page: u32,
}

impl Default for WorkshopState {
    fn default() -> Self {
        Self {
            tab: WorkshopTab::default(),
            preview: None,
            published_file_id: String::new(),
            publisher_account: String::new(),
            change_note: String::new(),
            confirmation: String::new(),
            selected_items: Default::default(),
            recent_acquisitions: Vec::new(),
            pending_acquisition_ids: Default::default(),
            service_review_id: None,
            omitted_review_file_paths: 0,
            search_query: String::new(),
            pending_search: None,
            service_search_revision: 0,
            pending_service_search_revision: None,
            service_acquisition_revision: 0,
            results: Vec::new(),
            focused_result: None,
            results_query: None,
            subscriptions: Default::default(),
            subscriptions_loaded: false,
            subscriptions_stale: false,
            service_subscriptions_revision: None,
            subscription_search_query: String::new(),
            subscription_details: Default::default(),
            subscription_poll_interval: WORKSHOP_PROGRESS_POLL_INTERVAL,
            next_subscription_poll: None,
            total_results: 0,
            page: 1,
        }
    }
}

enum RepreviewRequest {
    ActiveComposition,
    Source {
        selector: String,
        workspace_root: PathBuf,
    },
}

impl StudioApp {
    pub(super) fn apply_service_workshop_snapshot(
        &mut self,
        snapshot: &vapor_client_protocol::WorkshopSnapshot,
    ) {
        if let Some(search) = snapshot.search.as_ref() {
            let pending_matches = self
                .workshop
                .pending_search
                .as_ref()
                .is_some_and(|(query, page)| query == &search.query && *page == search.page);
            if pending_matches
                && self
                    .workshop
                    .pending_service_search_revision
                    .is_some_and(|revision| search.revision > revision)
            {
                self.workshop.service_search_revision = search.revision;
                self.workshop.pending_service_search_revision = None;
                self.apply_workshop_search(
                    vapor_client_application::WorkshopPage {
                        items: search
                            .items
                            .iter()
                            .map(|item| vapor_client_application::WorkshopItem {
                                published_file_id: item.published_file_id.clone(),
                                title: item.title.clone(),
                                description: item.description.clone(),
                                preview_url: item.preview_url.clone(),
                                updated_at_unix: item.updated_at_unix,
                            })
                            .collect(),
                        page: search.page,
                        total_results: search.total_results,
                    },
                    search.query.clone(),
                );
            } else {
                self.workshop.service_search_revision =
                    self.workshop.service_search_revision.max(search.revision);
            }
        }

        if snapshot.subscriptions_loaded
            && self.workshop.service_subscriptions_revision != Some(snapshot.subscriptions_revision)
        {
            let subscriptions = snapshot
                .subscriptions
                .iter()
                .map(|item| vapor_client_application::WorkshopSubscription {
                    published_file_id: item.published_file_id.clone(),
                    title: item.title.clone(),
                    description: item.description.clone(),
                    preview_url: item.preview_url.clone(),
                    updated_at_unix: item.updated_at_unix,
                    installed: item.installed,
                    needs_update: item.needs_update,
                    downloading: item.downloading,
                    download_progress: item.download_progress,
                })
                .collect();
            self.apply_workshop_subscriptions(subscriptions);
            self.workshop.service_subscriptions_revision = Some(snapshot.subscriptions_revision);
        }

        if let Some(batch) = snapshot
            .acquisition
            .as_ref()
            .filter(|batch| batch.revision > self.workshop.service_acquisition_revision)
        {
            let acquisitions = batch
                .items
                .iter()
                .map(|item| vapor_client_application::WorkshopAcquisition {
                    id: item.id.clone(),
                    version: item.version.clone(),
                    kind: item.kind.clone(),
                    cache_root: PathBuf::from(&item.cache_root),
                    fingerprint: item.fingerprint.clone(),
                })
                .collect();
            self.workshop.service_acquisition_revision = batch.revision;
            self.apply_workshop_acquisition(acquisitions, batch.requested);
        }

        if let Some(review) = snapshot.review.as_ref() {
            let preview = vapor_client_application::WorkshopPackagePreview {
                id: review.preview.id.clone(),
                version: review.preview.version.clone(),
                app_id: review.preview.app_id,
                published_file_id: review.preview.published_file_id.clone(),
                title: review.preview.title.clone(),
                visibility: review.preview.visibility.clone(),
                dependencies: review.preview.dependencies.clone(),
                root: PathBuf::from(&review.preview.root),
                target: review.preview.target.clone(),
                fingerprint: review.preview.fingerprint.clone(),
                files: review.preview.files,
                bytes: review.preview.bytes,
                file_paths: review.preview.file_paths.clone(),
            };
            self.workshop.service_review_id = Some(review.review_id.clone());
            self.workshop.omitted_review_file_paths = review.preview.omitted_file_paths;
            self.workshop.preview = Some(preview);
        } else if self.workshop.service_review_id.take().is_some() {
            self.workshop.preview = None;
            self.workshop.omitted_review_file_paths = 0;
        }
    }

    pub(super) fn review_workshop_content(&mut self, published_file_id: &str) {
        if let Some(selector) = self
            .catalog
            .items
            .iter()
            .find(|content| content.published_file_id.as_deref() == Some(published_file_id))
            .map(catalog_content_selector)
        {
            self.select_catalog_identity(selector);
        }
        self.catalog.source_filter = CatalogSourceFilter::Workshop;
        self.catalog.kind_filter = None;
        self.catalog.search_query = published_file_id.to_owned();
        self.open_discover_tab(super::catalog::DiscoverTab::Local);
    }

    fn catalog_identity_is_conflicted(&self, selector: &str) -> bool {
        self.catalog.items.iter().any(|content| {
            super::catalog_content_selector_matches(content, selector) && !content.is_unambiguous()
        })
    }

    pub(super) fn invalidate_workshop_review(&mut self) {
        let discard_service_review = self.workshop.service_review_id.take().is_some();
        self.workshop.preview = None;
        self.workshop.omitted_review_file_paths = 0;
        self.workshop.confirmation.clear();
        if discard_service_review && self.service.is_some() {
            self.submit_service_action(vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::DiscardReview,
            ));
        }
    }

    pub(super) fn apply_workshop_stage_error(&mut self, error: String) {
        self.invalidate_workshop_review();
        self.status = error;
    }

    pub(super) fn apply_workshop_error(&mut self, error: String) {
        if let Some((query, page)) = self.workshop.pending_search.take() {
            self.workshop.pending_service_search_revision = None;
            self.status = format!(
                "Workshop search for ‘{}’ (page {page}) failed: {error}. Previous results remain available.",
                if query.is_empty() {
                    "all items"
                } else {
                    &query
                },
            );
        } else {
            self.status = error;
        }
    }

    pub(super) fn apply_workshop_subscriptions_error(&mut self, error: String) {
        self.workshop.subscriptions_stale = self.workshop.subscriptions_loaded;
        self.status = error;
        let downloading = self
            .workshop
            .subscription_details
            .values()
            .any(|item| item.downloading);
        if downloading {
            let interval = self
                .workshop
                .subscription_poll_interval
                .saturating_mul(2)
                .min(WORKSHOP_PROGRESS_POLL_MAX_INTERVAL);
            self.workshop.subscription_poll_interval = interval;
            self.workshop.next_subscription_poll = Some(Instant::now() + interval);
        } else {
            self.workshop.next_subscription_poll = None;
        }
    }

    pub(super) fn apply_workshop_acquisition(
        &mut self,
        acquisitions: Vec<vapor_client_application::WorkshopAcquisition>,
        requested: usize,
    ) {
        let imported = acquisitions.len();
        for acquisition in acquisitions.into_iter().rev() {
            if let Some(existing) = self.workshop.recent_acquisitions.iter().position(|recent| {
                recent.id == acquisition.id && recent.version == acquisition.version
            }) {
                self.workshop.recent_acquisitions.remove(existing);
            }
            self.workshop.recent_acquisitions.insert(0, acquisition);
        }
        self.workshop
            .recent_acquisitions
            .truncate(MAX_RECENT_WORKSHOP_ACQUISITIONS);
        if imported == requested {
            self.activate_navigation_section(crate::navigation::Section::Library);
            self.status = if imported == 1 {
                "Added the Workshop item to content available on this device.".to_owned()
            } else {
                format!("Added {imported} Workshop items to content available on this device.")
            };
            for id in std::mem::take(&mut self.workshop.pending_acquisition_ids) {
                self.workshop.selected_items.remove(&id);
            }
        } else {
            self.open_workshop_tab(WorkshopTab::Imports);
            self.status = format!(
                "Downloaded {imported} of {requested} Workshop items. Your selection is still here so you can try again."
            );
            self.workshop.pending_acquisition_ids.clear();
        }
    }

    pub(super) fn apply_workshop_acquisition_error(&mut self, error: String) {
        self.workshop.pending_acquisition_ids.clear();
        self.status = error;
    }

    pub(super) fn apply_workshop_search(
        &mut self,
        page: vapor_client_application::WorkshopPage,
        query: String,
    ) {
        self.workshop.pending_search = None;
        self.workshop.pending_service_search_revision = None;
        self.workshop.page = page.page;
        self.workshop.total_results = page.total_results;
        self.workshop.results = page.items;
        if self.workshop.focused_result.as_ref().is_none_or(|focused| {
            !self
                .workshop
                .results
                .iter()
                .any(|item| &item.published_file_id == focused)
        }) {
            self.workshop.focused_result = self
                .workshop
                .results
                .first()
                .map(|item| item.published_file_id.clone());
        }
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
        subscriptions: Vec<vapor_client_application::WorkshopSubscription>,
    ) {
        self.workshop.subscriptions_loaded = true;
        self.workshop.subscriptions_stale = false;
        let downloading = subscriptions.iter().any(|item| item.downloading);
        self.workshop.subscriptions = subscriptions
            .iter()
            .map(|item| item.published_file_id.clone())
            .collect();
        self.workshop.subscription_details = subscriptions
            .into_iter()
            .map(|item| (item.published_file_id.clone(), item))
            .collect();
        self.workshop.subscription_poll_interval = WORKSHOP_PROGRESS_POLL_INTERVAL;
        self.workshop.next_subscription_poll =
            downloading.then(|| Instant::now() + WORKSHOP_PROGRESS_POLL_INTERVAL);
        self.status = format!(
            "Loaded {} Steam Workshop subscriptions.",
            self.workshop.subscriptions.len()
        );
    }

    pub(super) fn stage_workshop_content(&mut self, selector: String) {
        if !self.catalog.source_snapshot_current() || !self.catalog.local_catalog_current() {
            self.status = "Refresh the project list and on-device content list before preparing this upload. Studio needs current information to check the selected version and find conflicting copies.".to_owned();
            return;
        }
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot prepare {selector}: on-device copies of this version disagree. Review their details before continuing."
            );
            return;
        }
        if !self.require_open_workspace("stage this Workshop item") {
            return;
        }
        self.invalidate_workshop_review();
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::Stage { selector },
            ),
            None,
            OperationKind::StageWorkshop,
            vapor_client_application::OperationPhase::Staging,
            "Building and staging a Workshop review package…",
        );
    }

    pub(super) fn publish_reviewed_workshop_content(
        &mut self,
        selector: String,
        expected_review: vapor_client_application::WorkshopPackagePreview,
    ) {
        if !self.catalog.source_snapshot_current() || !self.catalog.local_catalog_current() {
            self.status = "Refresh the project list and on-device content list before publishing. Studio needs current information to check the selected version and find conflicting copies.".to_owned();
            return;
        }
        if self.catalog_identity_is_conflicted(&selector) {
            self.status = format!(
                "Cannot publish {selector}: on-device copies of this version disagree. Review their details before continuing."
            );
            return;
        }
        if !self.require_open_workspace("publish this Workshop item") {
            return;
        }
        if self.workshop.preview.as_ref() != Some(&expected_review) {
            self.status = "This staged review is no longer current. Stage the content again before publishing.".to_owned();
            self.invalidate_workshop_review();
            return;
        }
        let Some(review_id) = self.workshop.service_review_id.clone() else {
            self.status = "Studio no longer holds this Workshop review. Stage the content again before publishing.".to_owned();
            self.invalidate_workshop_review();
            return;
        };
        let account = self.workshop.publisher_account.trim().to_owned();
        if account.is_empty() {
            self.status =
                "Enter the Steam publisher account used by the local SteamCMD provider.".to_owned();
            return;
        }
        let expected_confirmation = format!("PUBLISH {selector} TO {account}");
        if self.workshop.confirmation.trim() != expected_confirmation {
            self.status =
                format!("Type {expected_confirmation} exactly to authorize this Workshop upload.");
            return;
        }
        let change_note = self.workshop.change_note.trim().to_owned();
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::Publish {
                    review_id,
                    account,
                    confirmation: self.workshop.confirmation.trim().to_owned(),
                    change_note,
                },
            ),
            None,
            OperationKind::PublishWorkshop,
            vapor_client_application::OperationPhase::Publishing,
            "Uploading the reviewed Workshop artifact through Vapor Shell…",
        );
    }

    pub(super) fn download_workshop_item(&mut self) {
        let published_file_id = self.workshop.published_file_id.trim().to_owned();
        if published_file_id.is_empty() {
            self.status = "Enter the number from the Workshop item's link.".to_owned();
            return;
        }
        if !published_file_id.bytes().all(|byte| byte.is_ascii_digit()) {
            self.status =
                "Enter the number from the Workshop item's link, not its full URL.".to_owned();
            return;
        }
        self.download_workshop_items(vec![published_file_id], None);
    }

    pub(super) fn download_selected_workshop_items(&mut self) {
        let ids = self
            .workshop
            .selected_items
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        self.download_workshop_items(ids, None);
    }

    pub(in crate::app) fn download_composition_dependencies(
        &mut self,
        published_file_ids: Vec<String>,
    ) {
        self.download_workshop_items(
            published_file_ids,
            Some(RepreviewRequest::ActiveComposition),
        );
    }

    pub(in crate::app) fn download_source_composition_dependencies(
        &mut self,
        published_file_ids: Vec<String>,
        selector: String,
        workspace_root: PathBuf,
    ) {
        self.download_workshop_items(
            published_file_ids,
            Some(RepreviewRequest::Source {
                selector,
                workspace_root,
            }),
        );
    }

    pub(in crate::app) fn import_workshop_items(&mut self, published_file_ids: Vec<String>) {
        self.download_workshop_items(published_file_ids, None);
    }

    fn download_workshop_items(
        &mut self,
        published_file_ids: Vec<String>,
        repreview_after_acquisition: Option<RepreviewRequest>,
    ) {
        if self.operation.running {
            self.status =
                "Finish the current operation before importing Workshop content.".to_owned();
            return;
        }
        if published_file_ids.is_empty() {
            self.status = "Select at least one Workshop item to import.".to_owned();
            return;
        }
        if !published_file_ids
            .iter()
            .all(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()))
        {
            self.status = "Enter the Workshop item numbers, not full links.".to_owned();
            return;
        }
        let count = published_file_ids.len();
        let pending_acquisition_ids = published_file_ids.iter().cloned().collect();
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::Acquire {
                    item_ids: published_file_ids,
                },
            ),
            None,
            OperationKind::Acquire,
            vapor_client_application::OperationPhase::Acquiring,
            &if count == 1 {
                "Downloading this item for use in Studio…".to_owned()
            } else {
                format!("Downloading {count} items for use in Studio…")
            },
        );
        if !self.operation.service_owned
            || !self
                .operation
                .active_operation
                .as_ref()
                .is_some_and(|operation| operation.kind == OperationKind::Acquire)
        {
            return;
        }
        self.workshop.pending_acquisition_ids = pending_acquisition_ids;
        if let Some(request) = repreview_after_acquisition {
            match request {
                RepreviewRequest::ActiveComposition => {
                    self.composition_preview
                        .request_repreview_after_catalog_refresh();
                }
                RepreviewRequest::Source {
                    selector,
                    workspace_root,
                } => {
                    self.composition_preview
                        .request_source_repreview_after_catalog_refresh(selector, workspace_root);
                }
            }
        }
        self.status = if count == 1 {
            "Downloading this item for use in Studio…".to_owned()
        } else {
            format!("Downloading {count} items for use in Studio…")
        };
    }

    pub(in crate::app) fn search_workshop(&mut self) {
        self.search_workshop_page(1);
    }

    pub(in crate::app) fn search_workshop_page(&mut self, page: u32) {
        if self.operation.running {
            self.status = "Wait for the current operation to finish.".to_owned();
            return;
        }
        let query = self.workshop.search_query.trim().to_owned();
        let status = if query.is_empty() {
            format!("Searching Steam Workshop · page {page}…")
        } else {
            format!("Searching Workshop for ‘{query}’ · page {page}…")
        };
        self.workshop.pending_search = Some((query.clone(), page));
        self.workshop.pending_service_search_revision = Some(self.workshop.service_search_revision);
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::Search { query, page },
            ),
            None,
            OperationKind::SearchWorkshop,
            vapor_client_application::OperationPhase::Resolving,
            &status,
        );
        self.status = status;
    }

    pub(in crate::app) fn refresh_workshop_subscriptions(&mut self) {
        if self.operation.running {
            return;
        }
        self.workshop.next_subscription_poll =
            Some(Instant::now() + self.workshop.subscription_poll_interval);
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(
                vapor_client_protocol::WorkshopAction::LoadSubscriptions,
            ),
            None,
            OperationKind::LoadWorkshopSubscriptions,
            vapor_client_application::OperationPhase::Resolving,
            "Loading Steam Workshop subscriptions…",
        );
        self.status = "Loading Steam Workshop subscriptions…".to_owned();
    }

    pub(super) fn poll_workshop_download_progress(&mut self) {
        let downloading = self
            .workshop
            .subscription_details
            .values()
            .any(|item| item.downloading);
        let due = self
            .workshop
            .next_subscription_poll
            .is_some_and(|at| Instant::now() >= at);
        if downloading && due {
            self.refresh_workshop_subscriptions();
        }
    }

    pub(in crate::app) fn set_workshop_subscription(&mut self, id: String, subscribed: bool) {
        self.set_workshop_subscriptions(vec![id], subscribed);
    }

    pub(in crate::app) fn set_workshop_subscriptions(
        &mut self,
        ids: Vec<String>,
        subscribed: bool,
    ) {
        if ids.is_empty() {
            self.status = "Select at least one Workshop item.".to_owned();
            return;
        }
        if ids.len() > vapor_client_application::MAX_WORKSHOP_SUBSCRIPTION_BATCH {
            self.status = format!(
                "Select no more than {} Workshop items for one subscription change.",
                vapor_client_application::MAX_WORKSHOP_SUBSCRIPTION_BATCH
            );
            return;
        }
        let mut unique = std::collections::BTreeSet::new();
        let mut normalized_ids = Vec::new();
        for id in ids {
            let Some(parsed) = id.parse::<u64>().ok().filter(|id| *id != 0) else {
                self.status = format!("Invalid Steam PublishedFileId: {id}");
                return;
            };
            if unique.insert(parsed) {
                normalized_ids.push(parsed.to_string());
            }
        }
        if normalized_ids.is_empty() {
            self.status = "Select at least one valid Workshop item.".to_owned();
            return;
        }
        let action = if subscribed {
            vapor_client_protocol::WorkshopAction::Subscribe {
                item_ids: normalized_ids,
            }
        } else {
            vapor_client_protocol::WorkshopAction::Unsubscribe {
                item_ids: normalized_ids,
            }
        };
        self.start_service_operation(
            vapor_client_protocol::ClientAction::Workshop(action),
            None,
            OperationKind::ChangeWorkshopSubscription,
            vapor_client_application::OperationPhase::Resolving,
            if subscribed {
                "Subscribing to Steam Workshop items…"
            } else {
                "Unsubscribing from Steam Workshop items…"
            },
        );
        self.status = if subscribed {
            "Subscribing to Steam Workshop items…"
        } else {
            "Unsubscribing from Steam Workshop items…"
        }
        .to_owned();
    }
}
