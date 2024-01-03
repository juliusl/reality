use std::time::{Duration, Instant};

use reality::prelude::*;

/// Provides methods for managing work state w/ thunk context cache
/// 
/// TODO: Move implementation to transient storage, add dep. repr level to tc/storage api
/// -- .as_ref().dep::<Progress>("progress_percent_f32"); // Returns dispatcher
/// 
pub trait WorkState: AsMut<ThunkContext> + AsRef<ThunkContext> {
    /// Sets the current progress,
    ///
    fn set_progress(&mut self, percent: f32) {
        self.as_mut()
            .store_kv("progress_percent_f32", Progress(percent));
    }

    /// Gets the current progress percent,
    ///
    fn get_progress(&self) -> Option<f32> {
        self.as_ref()
            .fetch_kv::<Progress>("progress_percent_f32")
            .map(|(_, p)| p.0)
    }

    /// Sets the status message that represents the work state,
    ///
    fn set_message(&mut self, status: impl Into<String>) {
        self.as_mut().store_kv(
            "progress_status_message_string",
            StatusMessage(status.into()),
        );
    }

    /// Get the current status message,
    ///
    fn get_message(&self) -> Option<String> {
        self.as_ref()
            .fetch_kv::<StatusMessage>("progress_status_message_string")
            .map(|(_, s)| s.0.to_string())
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
        self.set_progress(0.0);
        self.set_message("");
        self.set_work_start();
        self.set_work_stop();
    }

    /// Reset work state,
    ///
    fn reset(&mut self) {
        self.as_mut().delete_kv::<Progress>("progress_percent_f32");
        self.as_mut()
            .delete_kv::<StatusMessage>("progress_status_message_string");
        self.as_mut().delete_kv::<WorkStartTime>("work_start_time");
        self.as_mut().delete_kv::<WorkStopTime>("work_stop_time");
    }
}

struct Progress(f32);
struct StatusMessage(String);
struct WorkStartTime(Instant);
struct WorkStopTime(Instant);
impl WorkState for ThunkContext {}
