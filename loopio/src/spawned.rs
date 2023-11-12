use std::time::Instant;

use anyhow::anyhow;
use reality::ThunkContext;
use tokio::task::JoinHandle;
use tokio::sync::watch::Ref;

/// Various informal small commands to communicate across a message channel,
/// 
pub enum SmallCommand {
    /// Requests the listener to provide a status update,
    ///
    ProvideStatus,
    /// Requests the listener to provide current progress,
    ///
    ProvideProgress,
}

/// Struct enriching a join handle returned by an async runtime,
///
pub struct Spawned<O>
where
    O: Instruction + Send + Sync + 'static,
{
    /// Join handle to runtime running the task,
    ///
    pub spawned: JoinHandle<anyhow::Result<O>>,
    /// If set, has the most recent instruction status,
    ///
    pub status: Option<InstructionStatusMonitor>,
    /// Message channel to pass additional messages to a running instruction,
    ///
    pub messages: Option<InstructionProxy>,
}

impl<O> Spawned<O>
where
    O: Instruction + Send + Sync + 'static,
{
    /// Spawns an instruction and returns a new Spawned object,
    ///
    pub fn spawn(mut instruction: O) -> Self {
        Self {
            status: instruction.setup_status_updates(),
            messages: instruction.setup_instruction_proxy(),
            spawned: instruction.spawn(),
        }
    }

    /// Gets the current spawned status,
    ///
    pub fn status(&self) -> Option<Ref<InstructionStatus>> {
        self.status.as_ref().map(|s| s.rx.borrow())
    }

    /// Sends a command to the spawned instruction,
    ///
    pub async fn send_command(&self, command: SmallCommand) -> anyhow::Result<()> {
        if let Some(commands) = self.messages.as_ref() {
            Ok(commands.tx.send(command).await?)
        } else {
            Err(anyhow!("Commands are not enabled"))
        }
    }

    /// Returns true if the underlying instruction has completed,
    ///
    pub fn is_finished(&self) -> bool {
        self.spawned.is_finished()
    }
}

/// Various properties on operation status,
///
pub struct InstructionStatus {
    /// Time the instruction started,
    ///
    pub started: Instant,
    /// Time the looper finished,
    ///
    pub finished: Option<Instant>,
    /// Log buffer output,
    ///
    pub log_buffer: String,
    /// Current condition,
    ///
    pub condition: anyhow::Result<()>,
    /// If set, will be a floating point between 0.0 - 1.0 indicating
    /// the current progress,
    ///
    pub progress: Option<f64>,
}

impl InstructionStatus {
    /// Starts a new instruction status,
    ///
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            finished: None,
            log_buffer: String::new(),
            condition: Ok(()),
            progress: None,
        }
    }

    /// Sets the condition for the instruction status,
    ///
    pub fn set_condition(&mut self, condition: anyhow::Result<()>) {
        self.condition = condition;
    }

    /// Sets the current progress,
    ///
    pub fn set_progress(&mut self, progress: f64) {
        self.progress = Some(progress);
    }
}

impl std::fmt::Write for InstructionStatus {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.log_buffer.write_str(s)
    }
}

/// An instruction is a simple trait for passing a limited instruction set,
///
pub trait Instruction
where
    Self: Sized,
{
    /// Consume the current context and spawn a new task,
    /// 
    fn spawn(self) -> JoinHandle<anyhow::Result<Self>>;

    /// Returns a sender if the current instruction supports receiving messages,
    ///
    fn setup_instruction_proxy(&mut self) -> Option<InstructionProxy>;

    /// Returns a watch rx if the current instructions supports emitting status updates,
    ///
    fn setup_status_updates(&mut self) -> Option<InstructionStatusMonitor>;

    /// Updates the status from the running instruction,
    /// 
    fn update_status_from_instruction(&mut self, status: InstructionStatus) -> anyhow::Result<()>;

    /// Handle receiving a small command,
    /// 
    fn handle_small_command(&mut self) -> anyhow::Result<()>;
}

impl Instruction for ThunkContext {
    fn spawn(self) -> JoinHandle<anyhow::Result<Self>> {
        tokio::spawn(async move {
            if let Some(next) = self.call().await? {
                Ok(next)
            } else {
                Err(anyhow!("Could not start thunk"))
            }
        })
    }

    fn setup_instruction_proxy(&mut self) -> Option<InstructionProxy> {
        let (proxy, rx) = InstructionProxy::new(10000);

        self.write_cache(rx);

        Some(proxy)
    }

    fn setup_status_updates(&mut self) -> Option<InstructionStatusMonitor> {
        let (monitor, rx) = InstructionStatusMonitor::new();

        self.write_cache(rx);

        Some(monitor)
    }

    fn update_status_from_instruction(&mut self, next: InstructionStatus) -> anyhow::Result<()> {
        if let Some(cached) = self.cached_ref::<tokio::sync::watch::Sender<InstructionStatus>>() {
            cached.send(next)?;
        }

        Ok(())
    }

    /// Handles reading from the small command pipeline,
    /// 
    fn handle_small_command(&mut self) -> anyhow::Result<()> {
        if let Some(cached) = self
            .cached_mut::<tokio::sync::mpsc::Receiver<SmallCommand>>()
            .and_then(|mut c| c.try_recv().ok())
        {
            self.write_cache(cached);
        }

        Ok(())
    }
}

pub struct InstructionStatusMonitor {
    rx: tokio::sync::watch::Receiver<InstructionStatus>,
}

impl InstructionStatusMonitor {
    pub fn new() -> (Self, tokio::sync::watch::Sender<InstructionStatus>) {
        let (tx, rx) = tokio::sync::watch::channel(InstructionStatus::new());

        (Self { rx }, tx)
    }
}

pub struct InstructionProxy {
    tx: tokio::sync::mpsc::Sender<SmallCommand>,
}

impl InstructionProxy {
    pub fn new(buffer: usize) -> (Self, tokio::sync::mpsc::Receiver<SmallCommand>) {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);

        (Self { tx }, rx)
    }
}
