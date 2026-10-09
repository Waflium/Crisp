#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MoonPhase {
    FullMoon,
    WaningGibbous,
    ThirdQuarter,
    WaningCrescent,
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
}

impl MoonPhase {
    pub const VALUES: [MoonPhase; 8] = [
        MoonPhase::FullMoon,
        MoonPhase::WaningGibbous,
        MoonPhase::ThirdQuarter,
        MoonPhase::WaningCrescent,
        MoonPhase::NewMoon,
        MoonPhase::WaxingCrescent,
        MoonPhase::FirstQuarter,
        MoonPhase::WaxingGibbous,
    ];
    pub const PHASE_LENGTH: i32 = 24000;

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn start_tick(self) -> i32 {
        self.index() * Self::PHASE_LENGTH
    }

    pub fn name(self) -> &'static str {
        ["full_moon", "waning_gibbous", "third_quarter", "waning_crescent", "new_moon", "waxing_crescent", "first_quarter", "waxing_gibbous"][self as usize]
    }

    pub fn by_name(name: &str) -> Option<MoonPhase> {
        Self::VALUES.into_iter().find(|p| p.name() == name)
    }
}

pub const MOON_BRIGHTNESS_PER_PHASE: [f32; 8] = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
