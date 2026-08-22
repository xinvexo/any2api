use std::{sync::Mutex, time::Instant};

const WINDOW_SECONDS: u64 = 60;
const BUCKET_COUNT: usize = 60;

#[derive(Clone, Copy, Debug, Default)]
struct Bucket {
    second: Option<u64>,
    count: u64,
}

pub(super) struct PublicRequestWindow {
    origin: Instant,
    buckets: Mutex<[Bucket; BUCKET_COUNT]>,
}

impl PublicRequestWindow {
    pub(super) fn new() -> Self {
        Self::starting_at(Instant::now())
    }

    pub(super) fn record(&self) {
        let mut buckets = self.buckets.lock().expect("public request window");
        let second = self.elapsed_second(Instant::now());
        Self::record_second(&mut buckets, second);
    }

    pub(super) fn count(&self) -> u64 {
        let buckets = self.buckets.lock().expect("public request window");
        let second = self.elapsed_second(Instant::now());
        Self::count_at_second(&buckets, second)
    }

    fn starting_at(origin: Instant) -> Self {
        Self {
            origin,
            buckets: Mutex::new([Bucket::default(); BUCKET_COUNT]),
        }
    }

    fn elapsed_second(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.origin).as_secs()
    }

    fn record_second(buckets: &mut [Bucket; BUCKET_COUNT], second: u64) {
        let index = usize::try_from(second % WINDOW_SECONDS)
            .expect("public request window index fits usize");
        let bucket = &mut buckets[index];
        if bucket.second != Some(second) {
            *bucket = Bucket {
                second: Some(second),
                count: 0,
            };
        }
        bucket.count = bucket.count.saturating_add(1);
    }

    fn count_at_second(buckets: &[Bucket; BUCKET_COUNT], second: u64) -> u64 {
        buckets
            .iter()
            .filter(|bucket| {
                bucket
                    .second
                    .and_then(|bucket_second| second.checked_sub(bucket_second))
                    .is_some_and(|age| age < WINDOW_SECONDS)
            })
            .fold(0_u64, |total, bucket| total.saturating_add(bucket.count))
    }

    #[cfg(test)]
    fn record_at(&self, now: Instant) {
        let mut buckets = self.buckets.lock().expect("public request window");
        let second = self.elapsed_second(now);
        Self::record_second(&mut buckets, second);
    }

    #[cfg(test)]
    fn count_at(&self, now: Instant) -> u64 {
        let buckets = self.buckets.lock().expect("public request window");
        let second = self.elapsed_second(now);
        Self::count_at_second(&buckets, second)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };

    use super::PublicRequestWindow;

    #[test]
    fn counts_each_request_until_its_second_leaves_the_window() {
        let start = Instant::now();
        let window = PublicRequestWindow::starting_at(start);

        window.record_at(start);
        window.record_at(start + Duration::from_millis(900));
        window.record_at(start + Duration::from_secs(1));

        assert_eq!(window.count_at(start + Duration::from_secs(59)), 3);
        assert_eq!(window.count_at(start + Duration::from_secs(60)), 1);
        assert_eq!(window.count_at(start + Duration::from_secs(61)), 0);
    }

    #[test]
    fn reuses_a_bucket_without_retaining_the_previous_minute() {
        let start = Instant::now();
        let window = PublicRequestWindow::starting_at(start);

        window.record_at(start);
        window.record_at(start + Duration::from_secs(60));
        window.record_at(start + Duration::from_secs(60));

        assert_eq!(window.count_at(start + Duration::from_secs(60)), 2);
        assert_eq!(window.count_at(start + Duration::from_secs(120)), 0);
    }

    #[test]
    fn concurrent_records_are_not_lost() {
        let window = Arc::new(PublicRequestWindow::new());
        let workers = 8;
        let records_per_worker = 1_000;
        let mut threads = Vec::with_capacity(workers);

        for _ in 0..workers {
            let window = Arc::clone(&window);
            threads.push(thread::spawn(move || {
                for _ in 0..records_per_worker {
                    window.record();
                }
            }));
        }
        for worker in threads {
            worker.join().expect("request counter worker");
        }

        assert_eq!(window.count(), (workers * records_per_worker) as u64);
    }
}
