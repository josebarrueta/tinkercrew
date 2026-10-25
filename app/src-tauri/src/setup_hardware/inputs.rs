//! Evidence adapter for authenticated Companion state/events, never frontend claims.
use serde_json::Value;

pub(super) fn fresh_distance(state: &Value) -> Result<f64, String> {
    for name in ["buttons", "distance"] {
        if state["observables"][name]["health"] != "operational"
            || state["observables"][name]["age_ms"]
                .as_f64()
                .is_none_or(|age| !age.is_finite() || !(0.0..=750.0).contains(&age))
        {
            return Err(
                "Live input evidence is stale or missing; repeat the self-test after recovery."
                    .into(),
            );
        }
    }
    if state["observables"]["buttons"]["value"]
        .as_array()
        .is_none_or(|buttons| buttons.len() != 3 || !buttons.iter().all(Value::is_boolean))
    {
        return Err("Invalid live button evidence.".into());
    }
    state["observables"]["distance"]["value"]
        .as_f64()
        .filter(|distance| distance.is_finite() && (20.0..=4000.0).contains(distance))
        .ok_or("Invalid live distance evidence; zero/implausible samples cannot pass setup.".into())
}
pub(super) struct InputWitness {
    initial: f64,
    distance: f64,
    after: u64,
    pressed: u8,
    moved: bool,
    valid: bool,
}
impl InputWitness {
    pub fn new(state: &Value, history: &Value) -> Result<Self, String> {
        let distance = fresh_distance(state)?;
        let history = history
            .as_array()
            .ok_or("Invalid hardware event history.")?;
        let mut after: Option<u64> = None;
        for event in history {
            let sequence = event["sequence"]
                .as_u64()
                .filter(|sequence| {
                    *sequence > 0
                        && after.is_none_or(|previous| previous.checked_add(1) == Some(*sequence))
                })
                .ok_or("Invalid hardware event history sequence.")?;
            after = Some(sequence);
        }
        Ok(Self {
            initial: distance,
            distance,
            after: after.unwrap_or(0),
            pressed: 0,
            moved: false,
            valid: true,
        })
    }
    pub fn after(&self) -> u64 {
        self.after
    }
    pub fn buttons(&self) -> [bool; 3] {
        [
            self.pressed & 1 != 0,
            self.pressed & 2 != 0,
            self.pressed & 4 != 0,
        ]
    }
    pub fn moved(&self) -> bool {
        self.moved
    }
    pub fn distance(&self) -> f64 {
        self.distance
    }
    pub fn ready(&self) -> bool {
        self.valid && self.pressed == 7 && self.moved
    }
    pub fn observe(&mut self, state: &Value, events: &Value) -> Result<(), String> {
        if !self.valid {
            return Err("The input witness was invalidated; start a new self-test.".into());
        }
        let result = self.update(state, events);
        if result.is_err() {
            self.valid = false;
        }
        result
    }
    fn update(&mut self, state: &Value, events: &Value) -> Result<(), String> {
        self.distance = fresh_distance(state)?;
        self.moved |= (self.distance - self.initial).abs() >= 10.0;
        for event in events.as_array().ok_or("Invalid live event response.")? {
            let sequence = event["sequence"]
                .as_u64()
                .filter(|sequence| self.after.checked_add(1) == Some(*sequence))
                .ok_or("The hardware event window has a gap; repeat the test.")?;
            self.after = sequence;
            if event["type"] == "button.changed"
                && event["capability"] == "buttons"
                && event["health"] == "operational"
                && event["payload"]["pressed"] == true
                && let Some(button) = event["payload"]["button"]
                    .as_u64()
                    .filter(|button| *button < 3)
            {
                self.pressed |= 1 << button;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn state(distance: f64, age: f64) -> Value {
        json!({"observables":{"buttons":{"value":[false,false,false],"health":"operational","age_ms":age},"distance":{"value":distance,"health":"operational","age_ms":age}}})
    }
    #[test]
    fn accepts_actual_fractional_companion_sample_ages_without_reusing_history() {
        let mut witness=InputWitness::new(&state(150.0,0.125),&json!([{"sequence":41,"type":"button.changed","capability":"buttons","health":"operational","payload":{"button":0,"pressed":true}}])).unwrap();
        assert_eq!(witness.buttons(), [false; 3]);
        witness.observe(&state(175.0, 124.25), &json!([])).unwrap();
        assert!(witness.moved());
        assert!(!witness.ready());
    }
    fn button(sequence: u64, button: u8) -> Value {
        json!({"sequence":sequence,"type":"button.changed","capability":"buttons","health":"operational","payload":{"button":button,"pressed":true}})
    }
    #[test]
    fn requires_each_fresh_operational_press_and_real_distance_movement() {
        let mut witness = InputWitness::new(&state(150.0, 1.0), &json!([])).unwrap();
        let mut bad = button(2, 1);
        bad["health"] = json!("degraded");
        witness
            .observe(
                &state(159.0, 1.0),
                &json!([button(1, 0), bad, button(3, 2)]),
            )
            .unwrap();
        assert_eq!(witness.buttons(), [true, false, true]);
        assert!(!witness.ready());
        witness
            .observe(&state(160.0, 1.0), &json!([button(4, 1)]))
            .unwrap();
        assert!(witness.ready());
    }
    #[test]
    fn gaps_and_stale_samples_permanently_invalidate_the_witness() {
        let mut gap = InputWitness::new(&state(150.0, 1.0), &json!([])).unwrap();
        assert!(
            gap.observe(&state(175.0, 1.0), &json!([button(2, 0)]))
                .is_err()
        );
        assert!(
            gap.observe(
                &state(175.0, 1.0),
                &json!([button(1, 0), button(2, 1), button(3, 2)])
            )
            .is_err()
        );
        for age in [-0.1, 750.01, 1000.0] {
            let mut witness = InputWitness::new(&state(150.0, 1.0), &json!([])).unwrap();
            assert!(
                witness
                    .observe(
                        &state(175.0, age),
                        &json!([button(1, 0), button(2, 1), button(3, 2)])
                    )
                    .is_err()
            );
            assert!(!witness.ready());
            assert!(witness.observe(&state(175.0, 0.0), &json!([])).is_err());
        }
    }
    #[test]
    fn refuses_unknown_input_shapes_and_malformed_history() {
        for distance in [0.0, 19.0, 4001.0] {
            assert!(InputWitness::new(&state(distance, 0.0), &json!([])).is_err());
        }
        let mut missing = state(150.0, 0.0);
        missing["observables"]["distance"]["age_ms"] = Value::Null;
        assert!(InputWitness::new(&missing, &json!([])).is_err());
        assert!(InputWitness::new(&state(150.0, 0.0), &json!({})).is_err());
        assert!(
            InputWitness::new(&state(150.0, 0.0), &json!([button(5, 0), button(7, 1)])).is_err()
        );
    }
}
