use crisp_core::mth;

const PI: f64 = std::f64::consts::PI;

fn square(x: f32) -> f32 {
    x * x
}

fn cube(x: f32) -> f32 {
    x * x * x
}

pub fn in_back(x: f32) -> f32 {
    square(x) * (2.70158 * x - 1.70158)
}

pub fn in_bounce(x: f32) -> f32 {
    1.0 - out_bounce(1.0 - x)
}

pub fn in_cubic(x: f32) -> f32 {
    cube(x)
}

pub fn in_elastic(x: f32) -> f32 {
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    (-(2.0f64.powf(10.0 * x as f64 - 10.0)) * ((x as f64 * 10.0 - 10.75) * (PI * 2.0 / 3.0) as f32 as f64).sin()) as f32
}

pub fn in_expo(x: f32) -> f32 {
    if x == 0.0 { 0.0 } else { 2.0f64.powf(10.0 * x as f64 - 10.0) as f32 }
}

pub fn in_quart(x: f32) -> f32 {
    square(square(x))
}

pub fn in_quint(x: f32) -> f32 {
    square(square(x)) * x
}

pub fn in_sine(x: f32) -> f32 {
    1.0 - mth::cos((x * (PI / 2.0) as f32) as f64)
}

pub fn in_out_bounce(x: f32) -> f32 {
    if x < 0.5 { (1.0 - out_bounce(1.0 - 2.0 * x)) / 2.0 } else { (1.0 + out_bounce(2.0 * x - 1.0)) / 2.0 }
}

pub fn in_out_circ(x: f32) -> f32 {
    let x = x as f64;
    if x < 0.5 {
        ((1.0 - (1.0 - (2.0 * x).powf(2.0)).sqrt()) / 2.0) as f32
    } else {
        (((1.0 - (-2.0 * x + 2.0).powf(2.0)).sqrt() + 1.0) / 2.0) as f32
    }
}

pub fn in_out_cubic(x: f32) -> f32 {
    if x < 0.5 { 4.0 * cube(x) } else { (1.0 - (-2.0 * x as f64 + 2.0).powf(3.0) / 2.0) as f32 }
}

pub fn in_out_quad(x: f32) -> f32 {
    if x < 0.5 { 2.0 * square(x) } else { (1.0 - (-2.0 * x as f64 + 2.0).powf(2.0) / 2.0) as f32 }
}

pub fn in_out_quart(x: f32) -> f32 {
    if x < 0.5 { 8.0 * square(square(x)) } else { (1.0 - (-2.0 * x as f64 + 2.0).powf(4.0) / 2.0) as f32 }
}

pub fn in_out_quint(x: f32) -> f32 {
    if (x as f64) < 0.5 { 16.0 * x * x * x * x * x } else { (1.0 - (-2.0 * x as f64 + 2.0).powf(5.0) / 2.0) as f32 }
}

pub fn out_bounce(x: f32) -> f32 {
    if x < 0.36363637 {
        7.5625 * square(x)
    } else if x < 0.72727275 {
        7.5625 * square(x - 0.54545456) + 0.75
    } else if (x as f64) < 0.9090909090909091 {
        7.5625 * square(x - 0.8181818) + 0.9375
    } else {
        7.5625 * square(x - 0.95454544) + 0.984375
    }
}

pub fn out_elastic(x: f32) -> f32 {
    if x == 0.0 {
        0.0
    } else if x == 1.0 {
        1.0
    } else {
        (2.0f64.powf(-10.0 * x as f64) * ((x as f64 * 10.0 - 0.75) * (PI * 2.0 / 3.0) as f32 as f64).sin() + 1.0) as f32
    }
}

pub fn out_expo(x: f32) -> f32 {
    if x == 1.0 { 1.0 } else { 1.0 - 2.0f64.powf(-10.0 * x as f64) as f32 }
}

pub fn out_quad(x: f32) -> f32 {
    1.0 - square(1.0 - x)
}

pub fn out_quint(x: f32) -> f32 {
    1.0 - (1.0 - x as f64).powf(5.0) as f32
}

pub fn out_sine(x: f32) -> f32 {
    mth::sin((x * (PI / 2.0) as f32) as f64)
}

pub fn in_out_sine(x: f32) -> f32 {
    -(mth::cos((PI as f32 * x) as f64) - 1.0) / 2.0
}

pub fn out_back(x: f32) -> f32 {
    1.0 + 2.70158 * cube(x - 1.0) + 1.70158 * square(x - 1.0)
}

pub fn out_quart(x: f32) -> f32 {
    1.0 - square(square(1.0 - x))
}

pub fn out_cubic(x: f32) -> f32 {
    1.0 - cube(1.0 - x)
}

pub fn in_out_expo(x: f32) -> f32 {
    if x < 0.5 {
        if x == 0.0 { 0.0 } else { (2.0f64.powf(20.0 * x as f64 - 10.0) / 2.0) as f32 }
    } else if x == 1.0 {
        1.0
    } else {
        ((2.0 - 2.0f64.powf(-20.0 * x as f64 + 10.0)) / 2.0) as f32
    }
}

pub fn in_quad(x: f32) -> f32 {
    x * x
}

pub fn out_circ(x: f32) -> f32 {
    ((1.0 - square(x - 1.0)) as f64).sqrt() as f32
}

pub fn in_out_elastic(x: f32) -> f32 {
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    let c5 = std::f32::consts::PI * 4.0 / 9.0;
    let sin = ((20.0 * x as f64 - 11.125) * c5 as f64).sin();
    if x < 0.5 {
        (-(2.0f64.powf(20.0 * x as f64 - 10.0) * sin) / 2.0) as f32
    } else {
        (2.0f64.powf(-20.0 * x as f64 + 10.0) * sin / 2.0 + 1.0) as f32
    }
}

pub fn in_circ(x: f32) -> f32 {
    (-((1.0 - x * x) as f64).sqrt()) as f32 + 1.0
}

pub fn in_out_back(x: f32) -> f32 {
    if x < 0.5 {
        return 4.0 * x * x * (7.189819 * x - 2.5949094) / 2.0;
    }
    let dt = 2.0 * x - 2.0;
    (dt * dt * (3.5949094 * dt + 2.5949094) + 2.0) / 2.0
}
