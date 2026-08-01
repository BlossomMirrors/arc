use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

pub struct DownloadQueue {
    semaphore: Arc<Semaphore>,
    limit: Mutex<usize>,
}

impl DownloadQueue {
    pub fn new(limit: usize) -> Self {
        let limit = limit.max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
            limit: Mutex::new(limit),
        }
    }

    pub async fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.semaphore.clone().acquire_owned().await.ok()
    }

    pub async fn set_limit(&self, new: usize) {
        let new = new.max(1);
        let mut current = self.limit.lock().await;
        if new > *current {
            self.semaphore.add_permits(new - *current);
        } else if new < *current {
            let surplus = *current - new;
            let retired = self.semaphore.forget_permits(surplus);
            if retired < surplus {
                let missing = (surplus - retired) as u32;
                let semaphore = self.semaphore.clone();
                tokio::spawn(async move {
                    if let Ok(permit) = semaphore.acquire_many_owned(missing).await {
                        permit.forget();
                    }
                });
            }
        }
        *current = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    async fn slot_free(queue: &DownloadQueue) -> Option<OwnedSemaphorePermit> {
        timeout(Duration::from_millis(50), queue.acquire())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    async fn hands_out_no_more_slots_than_the_limit() {
        let queue = DownloadQueue::new(2);
        let _a = slot_free(&queue).await.expect("first slot");
        let _b = slot_free(&queue).await.expect("second slot");
        assert!(slot_free(&queue).await.is_none());
    }

    #[tokio::test]
    async fn raising_the_limit_frees_slots_right_away() {
        let queue = DownloadQueue::new(1);
        let _a = slot_free(&queue).await.expect("first slot");
        assert!(slot_free(&queue).await.is_none());

        queue.set_limit(3).await;
        let _b = slot_free(&queue).await.expect("second slot");
        let _c = slot_free(&queue).await.expect("third slot");
        assert!(slot_free(&queue).await.is_none());
    }

    #[tokio::test]
    async fn lowering_the_limit_retires_slots_as_they_come_back() {
        let queue = DownloadQueue::new(3);
        let a = slot_free(&queue).await.expect("first slot");
        let b = slot_free(&queue).await.expect("second slot");
        let _c = slot_free(&queue).await.expect("third slot");

        queue.set_limit(1).await;
        tokio::task::yield_now().await;

        drop(a);
        drop(b);
        assert!(slot_free(&queue).await.is_none());
    }
}
