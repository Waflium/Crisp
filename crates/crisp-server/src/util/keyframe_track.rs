use serde_json::Value as Json;

use super::easing_type::EasingType;
use crate::world::attribute::{LerpFunction, Value};

#[derive(Clone, PartialEq, Debug)]
pub struct KeyframeTrack {
    pub keyframes: Vec<(i32, Value)>,
    pub easing_type: EasingType,
}

impl KeyframeTrack {
    pub fn parse(json: &Json, parse_value: impl Fn(&Json) -> Result<Value, String>) -> Result<KeyframeTrack, String> {
        let list = json.get("keyframes").and_then(Json::as_array).ok_or("missing keyframes")?;
        let mut keyframes = Vec::with_capacity(list.len());
        for keyframe in list {
            let ticks = keyframe.get("ticks").and_then(Json::as_i64).filter(|t| (0..=i32::MAX as i64).contains(t)).ok_or("keyframe ticks must be non-negative")?;
            keyframes.push((ticks as i32, parse_value(keyframe.get("value").ok_or("missing keyframe value")?)?));
        }
        Self::validate_keyframes(&keyframes)?;
        let easing_type = json.get("ease").map_or(Ok(EasingType::LINEAR), EasingType::parse)?;
        Ok(KeyframeTrack { keyframes, easing_type })
    }

    fn validate_keyframes(keyframes: &[(i32, Value)]) -> Result<(), String> {
        if keyframes.is_empty() {
            return Err("Keyframes must not be empty".into());
        }
        if !keyframes.is_sorted_by_key(|k| k.0) {
            return Err("Keyframes must be ordered by ticks field".into());
        }
        if keyframes.len() > 1 {
            let mut repeat_count = 0;
            let mut last_ticks = keyframes.last().unwrap().0;
            for &(ticks, _) in keyframes {
                if ticks == last_ticks {
                    repeat_count += 1;
                    if repeat_count > 2 {
                        return Err(format!("More than 2 keyframes on same tick: {ticks}"));
                    }
                } else {
                    repeat_count = 0;
                }
                last_ticks = ticks;
            }
        }
        Ok(())
    }

    pub fn validate_period(&self, period_ticks: i32) -> Result<(), String> {
        match self.keyframes.iter().find(|k| k.0 < 0 || k.0 > period_ticks) {
            Some(k) => Err(format!("Keyframe at tick {} must be in range [0; {period_ticks}]", k.0)),
            None => Ok(()),
        }
    }

    pub fn bake_sampler(&self, period_ticks: Option<i32>, lerp: LerpFunction) -> KeyframeTrackSampler {
        KeyframeTrackSampler::new(self, period_ticks, lerp)
    }
}

#[derive(Clone, Debug)]
struct Segment {
    easing: EasingType,
    from_value: Value,
    from_ticks: i32,
    to_value: Value,
    to_ticks: i32,
}

#[derive(Clone, Debug)]
pub struct KeyframeTrackSampler {
    period_ticks: Option<i32>,
    lerp: LerpFunction,
    segments: Vec<Segment>,
}

impl KeyframeTrackSampler {
    fn new(track: &KeyframeTrack, period_ticks: Option<i32>, lerp: LerpFunction) -> KeyframeTrackSampler {
        KeyframeTrackSampler { period_ticks, lerp, segments: Self::bake_segments(track, period_ticks) }
    }

    fn bake_segments(track: &KeyframeTrack, period_ticks: Option<i32>) -> Vec<Segment> {
        let keyframes = &track.keyframes;
        let segment = |from: &(i32, Value), from_ticks: i32, to: &(i32, Value), to_ticks: i32| Segment {
            easing: track.easing_type,
            from_value: from.1.clone(),
            from_ticks,
            to_value: to.1.clone(),
            to_ticks,
        };
        if keyframes.len() == 1 {
            let value = keyframes[0].1.clone();
            return vec![Segment { easing: EasingType::CONSTANT, from_value: value.clone(), from_ticks: 0, to_value: value, to_ticks: 0 }];
        }
        let mut segments = Vec::new();
        let (first, last) = (&keyframes[0], keyframes.last().unwrap());
        if let Some(period) = period_ticks {
            segments.push(segment(last, last.0 - period, first, first.0));
        }
        for pair in keyframes.windows(2) {
            segments.push(segment(&pair[0], pair[0].0, &pair[1], pair[1].0));
        }
        if let Some(period) = period_ticks {
            segments.push(segment(last, last.0, first, first.0 + period));
        }
        segments
    }

    pub fn sample(&self, ticks: i64) -> Value {
        let sample_ticks = match self.period_ticks {
            Some(period) => ticks.rem_euclid(period as i64),
            None => ticks,
        };
        let segment = self.segments.iter().find(|s| sample_ticks < s.to_ticks as i64).unwrap_or(self.segments.last().unwrap());
        if sample_ticks <= segment.from_ticks as i64 {
            return segment.from_value.clone();
        }
        if sample_ticks >= segment.to_ticks as i64 {
            return segment.to_value.clone();
        }
        let alpha = (sample_ticks - segment.from_ticks as i64) as f32 / (segment.to_ticks - segment.from_ticks) as f32;
        let eased_alpha = segment.easing.apply(alpha);
        self.lerp.apply(eased_alpha, &segment.from_value, &segment.to_value)
    }
}
