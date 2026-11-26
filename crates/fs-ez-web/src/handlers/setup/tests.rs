use std::sync::{Arc, Barrier};

use super::*;

#[tokio::test]
async fn setup_gate_admits_a_single_holder() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let state = crate::test_support::state(&temp);
    let first = state
        .setup_gate
        .clone()
        .try_acquire_owned()
        .expect("créneau libre");
    assert!(state.setup_gate.clone().try_acquire_owned().is_err());
    drop(first);
    assert!(state.setup_gate.clone().try_acquire_owned().is_ok());
}

#[tokio::test]
async fn setup_permit_outlives_the_request_future() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let state = crate::test_support::state(&temp);
    let gate = Arc::clone(&state.setup_gate);
    let permit = gate.clone().try_acquire_owned().expect("créneau libre");
    let barrier = Arc::new(Barrier::new(2));
    let task_barrier = Arc::clone(&barrier);
    // Même forme que le handler : le permis est déplacé dans la tâche bloquante.
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        task_barrier.wait();
    });
    // Abandon de la future HTTP : le JoinHandle tombe, la tâche bloquante continue.
    drop(task);
    assert!(gate.clone().try_acquire_owned().is_err());
    barrier.wait();
    for _ in 0..100 {
        if gate.clone().try_acquire_owned().is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("permis jamais rendu après la tâche bloquante");
}

#[test]
fn username_rules_match_the_login_field() {
    assert!(valid_username("alice"));
    assert!(valid_username("personne-42"));
    assert!(!valid_username(""));
    assert!(!valid_username("a\nb"));
    assert!(!valid_username(&"a".repeat(MAX_USERNAME_CHARS + 1)));
}
