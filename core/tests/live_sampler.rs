//! Live integration test: the sampler must run against the real system without panicking
//! and produce a self-consistent snapshot.

use pulsar_core::config::Config;
use pulsar_core::sampler::Sampler;

#[test]
fn two_ticks_are_consistent_and_do_not_panic() {
    let mut s = Sampler::new(Config::default());
    let a = s.tick();
    // Give the counters a moment to move.
    std::thread::sleep(std::time::Duration::from_millis(60));
    let b = s.tick();

    assert!(b.seq > a.seq);
    assert!(b.proc_count > 0, "expected to see processes");
    assert!(!b.groups.is_empty(), "expected at least one group");

    // The memory story must reconcile.
    assert_eq!(b.mem.used_real + b.mem.available, b.mem.total);
    assert!(b.sum_pss <= b.mem.used_real + b.mem.cache_reclaimable + b.mem.kernel);

    // Unattributed is exactly what the UI will show; the books must close.
    assert_eq!(b.mem.kernel + b.sum_pss + b.unattributed_mem(), b.mem.used_real);

    // CPU heat grid should cover the logical CPUs.
    assert!(!b.cpu.cores.is_empty());
    let max = b.cpu.cores.iter().map(|c| c.usage_pct).fold(0.0, f64::max);
    assert!(max <= 100.0 + 1e-6);

    // Every group's members must exist in the process list.
    for g in &b.groups {
        for pid in &g.pids {
            assert!(b.process_by_pid(*pid).is_some(), "group references missing pid");
        }
    }

    // Power is always labelled and never negative.
    assert!(b.power.total_w >= 0.0);
}
