//! Controller arithmetic and motion, independent of the window and renderer.
use anyhow::{bail, ensure, Result};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{json, Map, Value};
use std::fmt;

pub const IDS: [&str; 8] = [
    "reactor",
    "beamweapons",
    "missilesystem",
    "maneuver",
    "impulse",
    "warp",
    "frontshield",
    "rearshield",
];
pub const LABELS: [&str; 8] = [
    "REACTOR",
    "BEAM WEAPONS",
    "MISSILE SYSTEM",
    "MANEUVERING",
    "IMPULSE ENGINES",
    "WARP DRIVE",
    "FRONT SHIELDS",
    "REAR SHIELDS",
];
pub type Bank = [i32; 8];

// serde_json's Value normally accepts duplicate keys. Reject them recursively,
// including unknown extension properties, just like the C++ configuration reader.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON value")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(Value::Number(n)))
                    .ok_or_else(|| E::custom("Non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut values = Map::new();
                while let Some((k, Unique(v))) = a.next_entry::<String, Unique>()? {
                    if values.insert(k.clone(), v).is_some() {
                        return Err(de::Error::custom(format!("Duplicate JSON property: {k}")));
                    }
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        d.deserialize_any(V)
    }
}
pub fn parse_json(bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.len() <= 1024 * 1024, "Configuration exceeds 1 MiB");
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let Unique(v) = serde_json::from_slice(bytes)?;
    fn depth(v: &Value, n: usize) -> Result<()> {
        ensure!(n <= 64, "JSON nesting exceeds 64");
        match v {
            Value::Array(a) => {
                for v in a {
                    depth(v, n + 1)?;
                }
            }
            Value::Object(o) => {
                for v in o.values() {
                    depth(v, n + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    depth(&v, 0)?;
    Ok(v)
}
pub fn integer(v: &Value, lo: i32, hi: i32) -> Result<i32> {
    if let Some(n) = v.as_f64() {
        if n.is_finite() && n.fract() == 0.0 && n >= lo as f64 && n <= hi as f64 {
            return Ok(n as i32);
        }
    }
    bail!("Expected whole number in range {lo}..{hi}")
}
#[derive(Clone, Debug)]
pub struct Configuration {
    pub ship: String,
    pub power: Bank,
    pub coolant: Bank,
}
impl Configuration {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let v = parse_json(bytes)?;
        integer(&v["version"], 1, 1)?;
        let ship = v["shipName"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("shipName must be text"))?;
        ensure!(
            ship.as_bytes().iter().any(|b| *b > 32)
                && ship.encode_utf16().count() <= 60
                && !ship.contains('\0'),
            "shipName must contain 1-60 characters"
        );
        let mut power = [0; 8];
        let mut coolant = [0; 8];
        for (i, id) in IDS.iter().enumerate() {
            power[i] = integer(&v["defaults"]["power"][id], 0, 300)?;
            coolant[i] = integer(&v["defaults"]["coolant"][id], 0, 300)?;
        }
        Ok(Self {
            ship: ship.into(),
            power,
            coolant,
        })
    }
}
pub fn position(x: f64) -> i32 {
    assert!(x.is_finite(), "Non-finite slider position");
    x.round().clamp(0.0, 300.0) as i32
}
pub fn snap(x: f64) -> i32 {
    let n = position(x);
    for stop in [100, 150, 200, 250, 300] {
        if (n - stop).abs() <= 3 {
            return stop;
        }
    }
    let nearest = (n as f64 / 5.0).round() as i32 * 5;
    if (n - nearest).abs() <= 1 {
        nearest
    } else {
        n
    }
}
pub struct Allocation {
    pub total: i32,
    pub maximum: f64,
    pub percentages: [f64; 8],
}
pub fn allocate(weights: &Bank) -> Allocation {
    let total = weights.iter().sum::<i32>();
    let d = total.max(300) as f64;
    Allocation {
        total,
        maximum: 300.0 / d * 100.0,
        percentages: weights.map(|w| w as f64 / d * 100.0),
    }
}
#[derive(Clone)]
pub struct State {
    pub config: Configuration,
    pub power: Bank,
    pub coolant: Bank,
    pub cooling: bool,
}
impl State {
    pub fn new(config: Configuration) -> Self {
        Self {
            power: config.power,
            coolant: config.coolant,
            config,
            cooling: false,
        }
    }
    pub fn bank(&self) -> &Bank {
        if self.cooling {
            &self.coolant
        } else {
            &self.power
        }
    }
    pub fn bank_mut(&mut self) -> &mut Bank {
        if self.cooling {
            &mut self.coolant
        } else {
            &mut self.power
        }
    }
    pub fn set(&mut self, i: usize, x: f64) -> bool {
        let n = position(x);
        let changed = self.bank()[i] != n;
        self.bank_mut()[i] = n;
        changed
    }
    pub fn commit(&mut self, i: usize) {
        self.bank_mut()[i] = snap(self.bank()[i] as f64);
    }
    pub fn reset(&mut self) {
        *self.bank_mut() = if self.cooling {
            self.config.coolant
        } else {
            self.config.power
        };
    }
    pub fn payload(&self, sequence: u64, final_state: bool) -> String {
        fn object<T: Into<Value> + Copy>(a: &[T; 8]) -> Value {
            Value::Object(
                IDS.iter()
                    .zip(a)
                    .map(|(k, v)| (k.to_string(), (*v).into()))
                    .collect(),
            )
        }
        let a = allocate(&self.coolant);
        json!({"version":1,"shipName":self.config.ship,"sequence":sequence,"final":final_state,"powerPercentages":object(&self.power),"coolantWeights":object(&self.coolant),"coolant":{"totalWeight":a.total,"rulerMaximum":a.maximum,"percentages":object(&a.percentages)}}).to_string()
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    pub start: f64,
    pub target: f64,
    pub since: f64,
    pub active: bool,
}
impl Motion {
    pub fn duration(&self) -> f64 {
        (self.target - self.start).abs().min(300.0) / 300.0 * 170.0 + 80.0
    }
    pub fn sample(&self, now: f64) -> f64 {
        if !self.active {
            return self.target;
        }
        let d = self.target - self.start;
        let t = d.abs().min(300.0) / 300.0 * 170.0;
        let times = [0.0, t, t + 24.0, t + 44.0, t + 62.0, t + 80.0];
        let values = [
            self.start,
            self.target + d * 0.1,
            self.target - d * 0.01,
            self.target + d * 0.001,
            self.target - d * 0.0001,
            self.target,
        ];
        for i in 1..6 {
            if now - self.since < times[i] {
                return values[i - 1]
                    + (values[i] - values[i - 1])
                        * ((now - self.since - times[i - 1]) / (times[i] - times[i - 1]))
                            .clamp(0.0, 1.0);
            }
        }
        self.target
    }
    pub fn retarget(&mut self, value: f64, now: f64, animate: bool) {
        self.start = self.sample(now);
        self.target = value;
        self.since = now;
        self.active = animate && (self.target - self.start).abs() > 0.01;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpp_golden_parity() {
        let v: Value = serde_json::from_str(include_str!("../tests/cpp-golden.json")).unwrap();
        for c in v["positions"].as_array().unwrap() {
            let x = c[0].as_f64().unwrap();
            assert_eq!(position(x), c[1].as_i64().unwrap() as i32);
            assert_eq!(snap(x), c[2].as_i64().unwrap() as i32);
        }
        for c in v["motions"].as_array().unwrap() {
            let m = Motion {
                start: c[0].as_f64().unwrap(),
                target: c[1].as_f64().unwrap(),
                since: 0.0,
                active: true,
            };
            for (t, y) in c[2].as_array().unwrap().iter().enumerate() {
                assert!(
                    (m.sample(t as f64) - y.as_f64().unwrap()).abs() < 1e-10,
                    "{m:?} t={t}"
                );
            }
        }
        for c in v["allocations"].as_array().unwrap() {
            let bank = std::array::from_fn(|i| c[0][i].as_i64().unwrap() as i32);
            let a = allocate(&bank);
            assert_eq!(a.total, c[1].as_i64().unwrap() as i32);
            assert!((a.maximum - c[2].as_f64().unwrap()).abs() < 1e-10);
            for i in 0..8 {
                assert!((a.percentages[i] - c[3][i].as_f64().unwrap()).abs() < 1e-10);
            }
        }
    }
    #[test]
    fn independent_banks_and_exact_reset() {
        let mut s = State::new(Configuration::parse(include_bytes!("../config.json")).unwrap());
        s.set(0, 147.0);
        s.commit(0);
        assert_eq!(s.power[0], 150);
        s.cooling = true;
        s.set(1, 300.0);
        s.reset();
        assert_eq!(s.coolant, [0; 8]);
        assert_eq!(s.power[0], 150);
        s.cooling = false;
        s.reset();
        assert_eq!(s.power, [100; 8]);
        let v: Value = serde_json::from_str(&s.payload(302, true)).unwrap();
        assert_eq!(v["sequence"], 302);
        assert_eq!(v["final"], true);
        assert_eq!(v["powerPercentages"]["reactor"], 100);
        let config = Configuration {
            ship: "Exact defaults".into(),
            power: [102; 8],
            coolant: [149; 8],
        };
        let mut s = State::new(config);
        s.set(0, 300.0);
        s.reset();
        assert_eq!(s.power, [102; 8]);
        s.cooling = true;
        s.set(1, 0.0);
        s.reset();
        assert_eq!(s.coolant, [149; 8]);
    }
    #[test]
    fn strict_json_and_config() {
        for s in [
            "{\"a\":1,\"a\":2}",
            "{\"extra\":{\"a\":1,\"a\":2}}",
            "[1,]",
            "01",
            "1e999",
            "\"\\ud800\"",
        ] {
            assert!(parse_json(s.as_bytes()).is_err(), "{s}");
        }
        assert!(parse_json(&[0xff]).is_err());
        assert!(parse_json(&vec![b' '; 1024 * 1024 + 1]).is_err());
        let base = include_str!("../config.json");
        for s in [
            base.replace("ESS Endeavour", ""),
            base.replace("ESS Endeavour", &"x".repeat(61)),
            base.replace("100", "301"),
            base.replace("100", "1.5"),
        ] {
            assert!(Configuration::parse(s.as_bytes()).is_err());
        }
        assert!(Configuration::parse(base.replace("100", "1e2").as_bytes()).is_ok());
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend_from_slice(base.as_bytes());
        assert!(Configuration::parse(&bom).is_ok());
        assert!(
            Configuration::parse(base.replace("ESS Endeavour", &"🚀".repeat(30)).as_bytes())
                .is_ok()
        );
        assert!(
            Configuration::parse(base.replace("ESS Endeavour", &"🚀".repeat(31)).as_bytes())
                .is_err()
        );
        let nesting = format!("{}0{}", "[".repeat(65), "]".repeat(65));
        assert!(parse_json(nesting.as_bytes()).is_err());
    }
}
