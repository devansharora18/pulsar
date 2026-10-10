//! Fuzz-ish: feed truncated, binary and adversarial `/proc` content to every parser and
//! assert none of them panic.

use pulsar_core::cpu::parse_proc_stat;
use pulsar_core::disk::parse_diskstats;
use pulsar_core::filesystem::parse_mounts;
use pulsar_core::meminfo::MemInfo;
use pulsar_core::power::{attribute_watts, model_watts};
use pulsar_core::pressure::Pressure;
use pulsar_core::process::{IoCounters, Stat, Statm, Status};
use pulsar_core::pss::SmapsRollup;
use pulsar_core::rapl::energy_delta;

fn inputs() -> Vec<String> {
    vec![
        "".to_string(),
        "\0\0\0".to_string(),
        "\u{1}\u{2}\u{3}".to_string(),
        ":\n:\n:\n".to_string(),
        "a=b=c=d=e".to_string(),
        "MemTotal:".to_string(),
        "MemTotal: -1 kB".to_string(),
        "Rss: 999999999999999999999 kB".to_string(),
        "1 ()".to_string(),
        "18446744073709551616".to_string(),
        "cpu 18446744073709551615 18446744073709551615 18446744073709551615".to_string(),
        "some avg10=NaN avg60=inf avg300=-1 total=abc".to_string(),
        "x".repeat(10_000),
    ]
}

#[test]
fn parsers_never_panic_on_hostile_input() {
    for s in inputs() {
        let _ = MemInfo::parse(&s);
        let _ = Pressure::parse(&s);
        let _ = Stat::parse(&s);
        let _ = Status::parse(&s);
        let _ = Statm::parse(&s);
        let _ = SmapsRollup::parse(&s);
        let _ = IoCounters::parse(&s);
        let _ = parse_proc_stat(&s);
        let _ = parse_diskstats(&s);
        let _ = parse_mounts(&s);
    }
}

#[test]
fn numeric_helpers_survive_extremes() {
    assert_eq!(energy_delta(u64::MAX, 0, u64::MAX), 0);
    assert!(
        attribute_watts(f64::INFINITY, 1, 1).is_finite()
            || attribute_watts(f64::INFINITY, 1, 1) > 0.0
    );
    assert!(model_watts(f64::NAN, 10.0).is_nan() || model_watts(f64::NAN, 10.0) >= 0.0);
    assert_eq!(attribute_watts(10.0, 0, 0), 0.0);
}
