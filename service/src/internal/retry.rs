use hyper::Error as HyperError;
use log::{error, warn};
use std::time::Duration;
use tower::{
    retry::{
        Policy,
        backoff::{Backoff, ExponentialBackoff, ExponentialBackoffMaker, MakeBackoff},
    },
    util::rng::HasherRng,
};

#[derive(Clone)]
pub struct RetryPolicy {
    backoff: ExponentialBackoff<HasherRng>,
    remaining: usize,
}

impl RetryPolicy {
    pub fn new(max_retries: usize) -> Self {
        let mut maker = ExponentialBackoffMaker::new(
            Duration::from_millis(200),
            Duration::from_secs(5),
            0.5,
            HasherRng::new(),
        )
        .expect("valid backoff config");

        Self {
            backoff: maker.make_backoff(),
            remaining: max_retries,
        }
    }
}

impl<Req: Clone, Res> Policy<Req, Res, HyperError> for RetryPolicy {
    type Future = tokio::time::Sleep;

    fn retry(
        &mut self,
        _req: &mut Req,
        result: &mut Result<Res, HyperError>,
    ) -> Option<Self::Future> {
        if result.is_err() {
            if self.remaining == 0 {
                error!("Backoff exhausted; not retrying further");
                return None;
            }

            self.remaining -= 1;
            let delay = self.backoff.next_backoff();
            warn!("Request failed, retrying in {:?}", delay);
            Some(delay)
        } else {
            None
        }
    }

    fn clone_request(&mut self, req: &Req) -> Option<Req> {
        Some(req.clone())
    }
}
