use futures::StreamExt;
use std::sync::Arc;

use crate::prelude::*;
use crate::repr::HANDLES;

/// Struct for linking together levels into a single representation,
///
#[derive(Default)]
pub struct Linker<I = CrcInterner>
where
    I: InternerFactory,
{
    /// Interner,
    ///
    interner: I,
    /// Vector of intern handles tags for each level of the current representation,
    ///
    levels: Vec<Tag<InternHandle, Arc<InternHandle>>>,
    /// Interning ready notifications,
    ///
    ready_notify: Vec<Arc<tokio::sync::Notify>>,
}

impl Linker<CrcInterner> {
    pub fn new<T: Send + Sync + 'static>() -> Self {
        Self::describe_resource::<T>()
    }
}

impl<I: InternerFactory + Default> Linker<I> {
    /// Constructs and returns a new representation,
    ///
    pub async fn link(&self) -> anyhow::Result<Repr> {
        use futures::TryStreamExt;

        tracing::trace!("Creating repr, waiting for background interning to catch up");
        // Since these levels aren't shared once the factory takes ownership,
        // notify_one will reserve a permit and Notified should return immediately
        for r in self.ready_notify.iter() {
            r.notified().await;
        }
        tracing::trace!("Background interning is all caught up");

        let tail = futures::stream::iter(self.levels.iter())
            .map(Ok::<_, anyhow::Error>)
            .try_fold(
                Tag::new(&HANDLES, Arc::new(InternHandle::default())),
                |from, to| async move {
                    let _ = from.link(to).await?;

                    Ok(to.clone())
                },
            )
            .await?;

        let tail = tail.value();

        if let Some(tail) = HANDLES.copy(&tail).await {
            Ok(Repr { tail })
        } else {
            Err(anyhow::anyhow!("Could not create representation"))
        }
    }

    /// Pushes a level to the current stack of levels,
    ///
    pub fn push_level(&mut self, level: impl Level) -> anyhow::Result<()> {
        // Configure a new handle
        let (ready, handle) = level.configure(&mut self.interner).result()?;

        self.ready_notify.push(ready);

        // Handle errors
        if let Some(last) = self.levels.last() {
            let flag = last.create_value.level_flags();

            if flag != LevelFlags::from_bits_truncate(handle.level_flags().bits() >> 1) {
                Err(anyhow::anyhow!("Expected next level"))?;
            }
        } else if handle.level_flags() != LevelFlags::ROOT {
            Err(anyhow::anyhow!("Expected root level"))?;
        }

        // Push the level to the stack
        self.levels.push(Tag::new(&HANDLES, Arc::new(handle)));

        Ok(())
    }

    /// Creates a new repr w/ the root as the ResourceLevel,
    ///
    #[inline]
    pub(crate) fn describe_resource<T: Send + Sync + 'static>() -> Self {
        let mut repr = Linker::default();

        repr.push_level(ResourceLevel::new::<T>())
            .expect("should be able to push since the repr is empty");

        repr
    }

    /// Returns the current representation level,
    ///
    #[inline]
    pub fn level(&self) -> usize {
        self.levels.len() - 1
    }
}
