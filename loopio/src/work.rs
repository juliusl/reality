use std::time::{Duration, Instant};

use reality::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::trace;

/// Provides methods for managing work state w/ thunk context cache
///
/// TODO: Move implementation to transient storage, add dep. repr level to tc/storage api
/// -- .as_ref().dep::<Progress>("progress_percent_f32"); // Returns dispatcher
///
pub trait WorkState: AsMut<ThunkContext> + AsRef<ThunkContext> {
    /// Sets the current progress,
    ///
    fn set_progress(&mut self, percent: f32) {
        let progress = Progress(percent);

        let mut work_state = self.as_mut().work_state_mut();

        work_state.progress = progress;
    }

    /// Gets the current progress percent,
    ///
    fn get_progress(&self) -> Option<f32> {
        self.as_ref().work_state_ref().map(|w| w.progress.0)
    }

    /// Sets the status message that represents the work state,
    ///
    fn set_message(&mut self, status: impl Into<String>) {
        let message = StatusMessage(status.into());

        let mut work_state = self.as_mut().work_state_mut();

        work_state.status = message;
    }

    /// Get the current status message,
    ///
    fn get_message(&self) -> Option<String> {
        self.as_ref()
            .work_state_ref()
            .map(|w| w.status.0.to_string())
    }

    /// Sets the start time for the current work state,
    ///
    fn set_work_start(&mut self) {
        self.as_mut()
            .store_kv::<WorkStartTime>("work_start_time", WorkStartTime(Instant::now()));
    }

    /// Returns the start time for the current work state,
    ///
    fn get_start_time(&self) -> Option<Instant> {
        self.as_ref()
            .fetch_kv::<WorkStartTime>("work_start_time")
            .map(|c| c.1 .0)
    }

    /// Sets the stop time of the current work state,
    ///
    fn set_work_stop(&mut self) {
        self.as_mut()
            .store_kv("work_stop_time", WorkStopTime(Instant::now()));
    }

    /// Gets the stop time of the current work state,
    ///
    fn get_stop_time(&self) -> Option<Instant> {
        self.as_ref()
            .fetch_kv::<WorkStopTime>("work_stop_time")
            .map(|c| c.1 .0)
    }

    /// Returns the current elapsed time for the current work state,
    ///
    fn elapsed(&self) -> Option<Duration> {
        match (self.get_start_time(), self.get_stop_time()) {
            (Some(start), None) => Some(start.elapsed()),
            (Some(start), Some(stop)) => Some(stop.duration_since(start)),
            _ => None,
        }
    }

    fn init(&mut self) {
        self.reset();
        self.set_work_start();
        self.as_mut().work_state_mut();
    }

    /// Reset work state,
    ///
    fn reset(&mut self) {
        self.as_mut().delete_cached::<PrivateWorkState>();
        self.as_mut().delete_cached::<PrivateProgress>();
        self.as_mut().delete_cached::<PrivateStatus>();
        self.as_mut().delete_kv::<WorkStartTime>("work_start_time");
        self.as_mut().delete_kv::<WorkStopTime>("work_stop_time");
    }
}

pub(crate) trait __WorkState: AsRef<ThunkContext> + AsMut<ThunkContext> {
    fn work_state_mut(
        &mut self,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, PrivateWorkState> {
        self.as_mut().maybe_write_cache(PrivateWorkState::default())
    }

    fn work_state_ref(
        &self,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, PrivateWorkState>> {
        self.as_ref().cached_ref()
    }

    fn virtual_work_state_ref(
        &self,
    ) -> Option<<Shared as StorageTarget>::BorrowResource<'_, VirtualPrivateWorkState>> {
        self.as_ref().cached_ref::<VirtualPrivateWorkState>()
    }

    fn virtual_work_state_mut(
        &mut self,
        init: Option<VirtualPrivateWorkState>,
    ) -> <Shared as StorageTarget>::BorrowMutResource<'_, VirtualPrivateWorkState> {
        let init = init.unwrap_or(VirtualPrivateWorkState::new(
            self.work_state_ref()
                .as_deref()
                .cloned()
                .unwrap_or_default(),
        ));
        self.as_mut()
            .maybe_write_cache::<VirtualPrivateWorkState>(init)
    }
}

impl __WorkState for ThunkContext {}

/// Private work state plugin
///
#[derive(Reality, Default, Debug, Clone)]
#[plugin_def(
    call = on_update
)]
pub(crate) struct PrivateWorkState {
    #[reality(derive_fromstr)]
    input: String,
    #[reality(virtual_only)]
    progress: Progress,
    #[reality(virtual_only)]
    status: StatusMessage,
}

pub(crate) type PrivateProgress = FieldRef<PrivateWorkState, Progress, Progress>;
pub(crate) type PrivateStatus = FieldRef<PrivateWorkState, StatusMessage, StatusMessage>;

async fn on_update(tc: &mut ThunkContext) -> anyhow::Result<()> {
    if let Some(latest) = tc.cached::<PrivateWorkState>() {
        trace!("on_update private progress {:?}", latest);
        let virt = tc.virtual_work_state_mut(Some(latest.clone().to_virtual()));

        virt.progress.edit_value(|_, v| {
            let changed = v.0 == latest.progress.0;
            v.0 = latest.progress.0.clone();
            changed
        });

        virt.status.edit_value(|_, v| {
            let changed = v.0 == latest.status.0;
            v.0 = latest.status.0.clone();
            changed
        });
    }

    Ok(())
}

#[derive(PartialEq, PartialOrd, Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct Progress(pub f32);

impl FromStr for Progress {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Progress(f32::from_str(s)?))
    }
}

#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
pub(crate) struct StatusMessage(String);

impl FromStr for StatusMessage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StatusMessage(s.to_string()))
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct WorkStartTime(Instant);

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct WorkStopTime(Instant);

impl WorkState for ThunkContext {}
