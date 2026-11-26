use std::net::Ipv4Addr;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use super::*;

fn ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

#[test]
fn five_failures_then_blocked_then_window_expires() {
    let start = Instant::now();
    let mut limiter = LoginLimiter::default();
    for _ in 0..5 {
        assert!(limiter.check_and_record(ip(), start));
    }
    assert!(!limiter.check_and_record(ip(), start));
    assert!(!limiter.check_and_record(ip(), start + Duration::from_secs(899)));
    assert!(limiter.check_and_record(ip(), start + Duration::from_secs(900)));
}

#[test]
fn success_clears_and_other_ips_are_independent() {
    let now = Instant::now();
    let other = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let mut limiter = LoginLimiter::default();
    for _ in 0..5 {
        assert!(limiter.check_and_record(ip(), now));
    }
    assert!(!limiter.check_and_record(ip(), now));
    assert!(limiter.check_and_record(other, now));
    limiter.clear(ip());
    assert!(limiter.check_and_record(ip(), now));
}

#[test]
fn concurrent_attempts_are_capped_by_the_window() {
    const ATTEMPTS: usize = 32;
    let start = Instant::now();
    let limiter = Arc::new(Mutex::new(LoginLimiter::default()));
    let barrier = Arc::new(Barrier::new(ATTEMPTS));
    let handles: Vec<_> = (0..ATTEMPTS)
        .map(|_| {
            let limiter = Arc::clone(&limiter);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                limiter
                    .lock()
                    .expect("limiteur")
                    .check_and_record(ip(), start)
            })
        })
        .collect();

    let allowed = handles
        .into_iter()
        .map(|handle| handle.join().expect("tentative"))
        .filter(|allowed| *allowed)
        .count();

    assert_eq!(allowed, MAX_FAILURES as usize);
    assert!(
        !limiter
            .lock()
            .expect("limiteur")
            .check_and_record(ip(), start)
    );
}
