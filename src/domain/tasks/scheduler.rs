use crate::domain::models::error::Error;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::sync::watch;
pub type JobFuture = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;
#[derive(Clone)]
pub struct ScheduledJob {
    pub name: &'static str,
    pub interval: Duration,
    pub run: Arc<dyn Fn() -> JobFuture + Send + Sync>,
}
pub async fn run(jobs: Vec<ScheduledJob>, stop: watch::Receiver<bool>) {
    let mut tasks = vec![];
    for job in jobs {
        let mut stop = stop.clone();
        tasks.push(tokio::spawn(async move {
   loop {
    if *stop.borrow(){break;}
    if (job.run)().await.is_err(){tracing::warn!(job=job.name,"Scheduled job failed");}
    tokio::select! { _=tokio::time::sleep(job.interval)=>{}, _=stop.changed()=>{break;} }
   }
  }));
    }
    for task in tasks {
        let _ = task.await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test(start_paused = true)]
    async fn fixed_clock_intervals_and_shutdown() {
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let job = ScheduledJob {
            name: "fixture",
            interval: Duration::from_secs(10),
            run: Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(()) })
            }),
        };
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(run(vec![job], rx));
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(10)).await;
        tokio::task::yield_now().await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
        tx.send(true).unwrap();
        task.await.unwrap();
    }
}
