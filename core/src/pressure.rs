//! `/proc/pressure/*` (PSI) parsing.
//!
//! PSI is the honest answer to "is memory actually under pressure", far more useful than a
//! percentage-of-total number. We surface `some` and `full` avg10/60/300 per resource.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PressureValue {
    pub avg10: f64,
    pub avg60: f64,
    pub avg300: f64,
    pub total_us: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Pressure {
    /// At least one task stalled.
    pub some: PressureValue,
    /// Every task stalled (memory/cpu only; `io` also exposes it).
    pub full: Option<PressureValue>,
}

impl Pressure {
    /// Parse `/proc/pressure/<resource>`. The `full` line is optional.
    pub fn parse(text: &str) -> Self {
        let mut p = Pressure::default();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let Some(kind) = it.next() else { continue };
            let mut v = PressureValue::default();
            for tok in it {
                let Some((k, val)) = tok.split_once('=') else {
                    continue;
                };
                match k {
                    "avg10" => v.avg10 = val.parse().unwrap_or(0.0),
                    "avg60" => v.avg60 = val.parse().unwrap_or(0.0),
                    "avg300" => v.avg300 = val.parse().unwrap_or(0.0),
                    "total" => v.total_us = val.parse().unwrap_or(0),
                    _ => {}
                }
            }
            match kind {
                "some" => p.some = v,
                "full" => p.full = Some(v),
                _ => {}
            }
        }
        p
    }

    pub fn read(resource: &str) -> crate::Result<Self> {
        Ok(Self::parse(&std::fs::read_to_string(format!(
            "/proc/pressure/{resource}"
        ))?))
    }

    /// A single 0..=100 gauge number: the 10-second memory some-average.
    pub fn gauge(&self) -> f64 {
        self.some.avg10.clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
some avg10=0.00 avg60=0.00 avg300=0.00 total=9079165
full avg10=0.00 avg60=0.00 avg300=0.00 total=8741471
";

    #[test]
    fn parses_some_and_full() {
        let p = Pressure::parse(FIXTURE);
        assert_eq!(p.some.total_us, 9079165);
        assert!(p.full.is_some());
        assert_eq!(p.full.unwrap().total_us, 8741471);
    }

    #[test]
    fn handles_missing_full_and_garbage() {
        let p = Pressure::parse("some avg10=1.5 avg60=x avg300=2 total=1\n");
        assert_eq!(p.some.avg10, 1.5);
        assert_eq!(p.some.avg60, 0.0);
        assert!(p.full.is_none());
        assert_eq!(Pressure::parse("").gauge(), 0.0);
    }
}
