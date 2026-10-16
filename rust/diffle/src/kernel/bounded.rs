//! Running many futures with a ceiling on how many are in flight.

use std::future::Future;

use futures::StreamExt;
use futures::stream;

/// Applies `work` to every item with at most `limit` running at once; results keep the
/// items' order. A `limit` of zero is read as one.
pub async fn map_limit<T, R, Fut>(items: Vec<T>, limit: usize, work: impl FnMut(T) -> Fut) -> Vec<R>
where
    Fut: Future<Output = R>,
{
    stream::iter(items)
        .map(work)
        .buffered(limit.max(1))
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn keeps_order_and_never_exceeds_the_limit() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let results = map_limit((0..20_u64).collect(), 3, |n| {
            let (running, peak) = (&running, &peak);
            async move {
                let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20 - n)).await;
                running.fetch_sub(1, Ordering::SeqCst);
                n * 2
            }
        })
        .await;
        assert_eq!(results, (0..20_u64).map(|n| n * 2).collect::<Vec<_>>());
        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_zero_limit_still_makes_progress() {
        let results = map_limit(vec![1, 2, 3], 0, |n| async move { n + 1 }).await;
        assert_eq!(results, [2, 3, 4]);
    }
}
