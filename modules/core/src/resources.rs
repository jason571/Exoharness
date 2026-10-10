use async_trait::async_trait;
use std::collections::BTreeMap;
use tokio::task::JoinHandle;

/// An extension-owned effect. Implementations must tolerate interrupted cleanup
/// and provide a synchronous Drop safety net for OS/process/task resources.
#[async_trait]
pub trait OwnedResource: Send {
    async fn release(&mut self) -> Result<(), String>;
}

struct Effect {
    name: String,
    resource: Box<dyn OwnedResource>,
}

#[derive(Default)]
pub(crate) struct Resources(BTreeMap<String, Vec<Effect>>);

impl Resources {
    pub(crate) fn own(
        &mut self,
        owner: &str,
        name: &str,
        resource: Box<dyn OwnedResource>,
    ) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("resource name must not be blank".into());
        }
        let effects = self.0.entry(owner.into()).or_default();
        if effects.iter().any(|effect| effect.name == name) {
            return Err(format!("resource already owned: {name}"));
        }
        effects.push(Effect {
            name: name.into(),
            resource,
        });
        Ok(())
    }

    pub(crate) async fn release(&mut self, owner: &str) -> Result<(), String> {
        if let Some(effects) = self.0.get_mut(owner) {
            while let Some(effect) = effects.last_mut() {
                effect
                    .resource
                    .release()
                    .await
                    .map_err(|error| format!("resource {}: {error}", effect.name))?;
                effects.pop();
            }
        }
        self.0.remove(owner);
        Ok(())
    }
}

/// Task ownership has both an awaited cleanup path and a Drop abort guard.
/// Dropping a cancelled release future retains the JoinHandle for another drain.
pub struct ManagedTask(Option<JoinHandle<()>>);

impl ManagedTask {
    pub fn new(task: JoinHandle<()>) -> Self {
        Self(Some(task))
    }
}

#[async_trait]
impl OwnedResource for ManagedTask {
    async fn release(&mut self) -> Result<(), String> {
        if let Some(task) = self.0.as_mut() {
            task.abort();
            let result = task.await;
            self.0.take();
            if let Err(error) = result
                && !error.is_cancelled()
            {
                return Err(format!("owned task failed: {error}"));
            }
        }
        Ok(())
    }
}

impl Drop for ManagedTask {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}
