use reality::prelude::*;

/// Manages the work state,
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
    fn get_message(&mut self) -> Option<String> {
        self.as_mut()
            .fetch_kv::<StatusMessage>("progress_status_message_string")
            .map(|(_, s)| s.0.to_string())
    }

    /// Resets the work state,
    ///
    fn reset(&mut self) {
        self.as_mut()
            .take_kv::<Progress>("progress_percent_f32");
        self.as_mut()
            .take_kv::<StatusMessage>("progress_status_message_string");
    }
}

struct Progress(f32);
struct StatusMessage(String);

impl WorkState for ThunkContext {}
