use std::sync::LazyLock;

use crate::random::RandomSource;

pub const PI: f32 = std::f64::consts::PI as f32;
pub const DEG_TO_RAD: f32 = (std::f64::consts::PI / 180.0) as f32;
pub const RAD_TO_DEG: f32 = 180.0 / std::f64::consts::PI as f32;

const SIN_SCALE: f64 = 10430.378350470453;

static SIN: LazyLock<Box<[f32; 65536]>> = LazyLock::new(|| {
    let mut table = Box::new([0.0; 65536]);
    for (i, v) in table.iter_mut().enumerate() {
        *v = (i as f64 / SIN_SCALE).sin() as f32;
    }
    table
});

pub fn sin(i: f64) -> f32 {
    SIN[((i * SIN_SCALE) as i64 & 0xFFFF) as usize]
}

pub fn cos(i: f64) -> f32 {
    SIN[((i * SIN_SCALE + 16384.0) as i64 & 0xFFFF) as usize]
}

pub fn sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

pub fn java_round(x: f64) -> i64 {
    let f = x.floor();
    if x - f >= 0.5 { f as i64 + 1 } else { f as i64 }
}

pub fn java_round_f32(x: f32) -> i32 {
    let f = x.floor();
    if x - f >= 0.5 { (f as i32).saturating_add(1) } else { f as i32 }
}

pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

pub fn floor_f(v: f32) -> i32 {
    v.floor() as i32
}

pub fn lfloor(v: f64) -> i64 {
    v.floor() as i64
}

pub fn ceil(v: f64) -> i32 {
    v.ceil() as i32
}

pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min { min } else { value.min(max) }
}

pub fn clamp_f(value: f32, min: f32, max: f32) -> f32 {
    if value < min { min } else { value.min(max) }
}

pub fn clamp_i(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

pub fn out_from_origin(origin: i32, lower_bound: i32, upper_bound: i32, step_size: i32) -> impl Iterator<Item = i32> {
    assert!(lower_bound <= upper_bound, "upperBound {upper_bound} expected to be > lowerBound {lower_bound}");
    assert!(step_size >= 1, "step size expected to be >= 1, was {step_size}");
    let clamped_origin = clamp_i(origin, lower_bound, upper_bound);
    std::iter::successors(Some(clamped_origin), move |&cursor| {
        let previous_was_negative = cursor <= clamped_origin;
        let current_distance = (clamped_origin - cursor).abs();
        let can_move_positive = clamped_origin + current_distance + step_size <= upper_bound;
        if !previous_was_negative || !can_move_positive {
            let attempted_step = clamped_origin - current_distance - if previous_was_negative { step_size } else { 0 };
            if attempted_step >= lower_bound {
                return Some(attempted_step);
            }
        }
        Some(clamped_origin + current_distance + step_size)
    })
    .take_while(move |&cursor| {
        let current_distance = (clamped_origin - cursor).abs();
        clamped_origin - current_distance >= lower_bound || clamped_origin + current_distance <= upper_bound
    })
}

pub fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn jmin_f(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        b
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn jmax_f(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        b
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn jclamp_f(value: f32, min: f32, max: f32) -> f32 {
    if value < min { min } else { jmin_f(value, max) }
}

pub fn jclamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min { min } else { jmin(value, max) }
}

pub fn java_signum_f(x: f32) -> f32 {
    if x == 0.0 || x.is_nan() { x } else { 1.0f32.copysign(x) }
}

pub fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

#[allow(clippy::too_many_arguments)]
pub fn lerp3_f(a1: f32, a2: f32, a3: f32, x000: f32, x100: f32, x010: f32, x110: f32, x001: f32, x101: f32, x011: f32, x111: f32) -> f32 {
    lerp_f(a3, lerp_f(a2, lerp_f(a1, x000, x100), lerp_f(a1, x010, x110)), lerp_f(a2, lerp_f(a1, x001, x101), lerp_f(a1, x011, x111)))
}

pub fn lerp(alpha: f64, p0: f64, p1: f64) -> f64 {
    p0 + alpha * (p1 - p0)
}

pub fn lerp_f(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

pub fn clamped_lerp(factor: f64, min: f64, max: f64) -> f64 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp(factor, min, max)
    }
}

pub fn wrap_degrees(angle: f32) -> f32 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn wrap_degrees_d(angle: f64) -> f64 {
    let mut a = angle % 360.0;
    if a >= 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

pub fn wrap_degrees_i(angle: i32) -> i32 {
    let mut a = angle % 360;
    if a >= 180 {
        a -= 360;
    }
    if a < -180 {
        a += 360;
    }
    a
}

pub fn degrees_difference(from: f32, to: f32) -> f32 {
    wrap_degrees(to - from)
}

pub fn degrees_difference_abs(angle_a: f32, angle_b: f32) -> f32 {
    degrees_difference(angle_a, angle_b).abs()
}

pub fn rotate_if_necessary(base: f32, target: f32, max_diff: f32) -> f32 {
    let delta = degrees_difference(base, target);
    target - clamp_f(delta, -max_diff, max_diff)
}

pub fn approach(current: f32, target: f32, increment: f32) -> f32 {
    let increment = increment.abs();
    if current < target { clamp_f(current + increment, current, target) } else { clamp_f(current - increment, target, current) }
}

pub fn approach_degrees(current: f32, target: f32, increment: f32) -> f32 {
    approach(current, current + degrees_difference(current, target), increment)
}

pub fn pack_degrees(angle: f32) -> i8 {
    floor_f(angle * 256.0 / 360.0) as i8
}

pub fn unpack_degrees(rot: i8) -> f32 {
    (rot as i32 * 360) as f32 / 256.0
}

pub fn positive_modulo(input: i32, m: i32) -> i32 {
    input.rem_euclid(m)
}

pub fn positive_modulo_d(input: f64, m: f64) -> f64 {
    (input % m + m) % m
}

pub fn frac(num: f64) -> f64 {
    num - lfloor(num) as f64
}

pub fn frac_f(num: f32) -> f32 {
    num - floor(num as f64) as f32
}

pub fn lerp_discrete(alpha: f32, p0: i32, p1: i32) -> i32 {
    let delta = p1 - p0;
    p0 + floor((alpha * (delta - 1) as f32) as f64) + if alpha > 0.0 { 1 } else { 0 }
}

pub fn sign(number: f64) -> i32 {
    if number == 0.0 {
        0
    } else if number > 0.0 {
        1
    } else {
        -1
    }
}

pub fn square(x: f64) -> f64 {
    x * x
}

pub fn abs_max(a: f64, b: f64) -> f64 {
    jmax(a.abs(), b.abs())
}

static ASIN_COS_TAB_BITS: [(u64, u64); 257] = [
    (0x0000000000000000, 0x3ff0000000000000),
    (0x3f700002aaabdddf, 0x3fefffeffffbfffe),
    (0x3f80000aaabdde0c, 0x3fefffbfffbfff80),
    (0x3f8800240091cfda, 0x3fefff6ffebbfa4e),
    (0x3f90002aabdde94c, 0x3feffefffbffdfff),
    (0x3f94005358ff0bd8, 0x3feffe6ff63b85e7),
    (0x3f980090091d9024, 0x3feffdbfebbe9360),
    (0x3f9c00e4be5f0304, 0x3feffcefda786870),
    (0x3fa000aabde0b9c8, 0x3feffbffbff7fec0),
    (0x3fa200f3229fdf2c, 0x3feffaef996bc4e8),
    (0x3fa4014d8ffaf8af, 0x3feff9bf63a1740b),
    (0x3fa601bc0922e634, 0x3feff86f1b05dfb0),
    (0x3fa8024091fdb0a9, 0x3feff6febba4bfea),
    (0x3faa02dd2f38ebf6, 0x3feff56e412875ab),
    (0x3fac0393e65c2c93, 0x3feff3bda6d9c950),
    (0x3fae0466bddb929c, 0x3feff1ece79fa355),
    (0x3fb002abde953619, 0x3fefeffbfdfebf1f),
    (0x3fb103347666f892, 0x3fefedeae41957e6),
    (0x3fb203ce2b380cd3, 0x3fefebb993aecf99),
    (0x3fb3047a02794911, 0x3fefe968061b4fc8),
    (0x3fb405390240e6fd, 0x3fefe6f634576477),
    (0x3fb5060c31541da5, 0x3fefe46416f790d1),
    (0x3fb606f49730ccc5, 0x3fefe1b1a62bddad),
    (0x3fb707f33c173a99, 0x3fefdeded9bf61d3),
    (0x3fb809092913e52e, 0x3fefdbeba917c3f5),
    (0x3fb90a3768096840, 0x3fefd8d80b34b63e),
    (0x3fba0b7f03ba78ac, 0x3fefd5a3f6af6b74),
    (0x3fbb0ce107d3f690, 0x3fefd24f61ba0590),
    (0x3fbc0e5e80f7172d, 0x3fefceda421efdb5),
    (0x3fbd0ff87cc3a7a5, 0x3fefcb448d40857c),
    (0x3fbe11b009e269b5, 0x3fefc78e3817e16e),
    (0x3fbf1386380f8b9a, 0x3fefc3b73734bca2),
    (0x3fc00abe0c129e1e, 0x3fefbfbf7ebc755f),
    (0x3fc08bc95e132e6f, 0x3fefbba7026962a7),
    (0x3fc10ce59ba4a8c4, 0x3fefb76db58a1299),
    (0x3fc18e134f0178af, 0x3fefb3138b008181),
    (0x3fc20f530308cc20, 0x3fefae987541497f),
    (0x3fc290a543442d6a, 0x3fefa9fc6652caa7),
    (0x3fc3120a9bed2f46, 0x3fefa53f4fcc4b79),
    (0x3fc3938399f32b5c, 0x3fefa06122d5118c),
    (0x3fc41510cb011423, 0x3fef9b61d0237250),
    (0x3fc496b2bd835ab9, 0x3fef964147fbdbbf),
    (0x3fc5186a00ade974, 0x3fef90ff7a2fd4d2),
    (0x3fc59a37248233ea, 0x3fef8b9c561cf5a1),
    (0x3fc61c1ab9d55d30, 0x3fef8617caabd6f6),
    (0x3fc69e1552567517, 0x3fef8071c64ef938),
    (0x3fc720278094cd3c, 0x3fef7aaa3701a270),
    (0x3fc7a251d80666ab, 0x3fef74c10a46b354),
    (0x3fc82494ed0e78fc, 0x3fef6eb62d27730d),
    (0x3fc8a6f1550413c4, 0x3fef68898c325199),
    (0x3fc92967a638db38, 0x3fef623b1379a09b),
    (0x3fc9abf877ffe0e8, 0x3fef5bcaae92424e),
    (0x3fca2ea462b4998e, 0x3fef553848924e81),
    (0x3fcab16bffc1f0dd, 0x3fef4e83cc0fad49),
    (0x3fcb344fe9a97c4d, 0x3fef47ad231ea746),
    (0x3fcbb750bc0acdef, 0x3fef40b437506b2f),
    (0x3fcc3a6f13aae84b, 0x3fef3998f1b1886c),
    (0x3fccbdab8e7bd466, 0x3fef325b3ac85e81),
    (0x3fcd4106cba45b08, 0x3fef2afafa9380f9),
    (0x3fcdc4816b87e25e, 0x3fef237818880fa4),
    (0x3fce481c0fce7134, 0x3fef1bd27b9002c4),
    (0x3fcecbd75b6cd8f0, 0x3fef140a0a086af1),
    (0x3fcf4fb3f2ad079b, 0x3fef0c1ea9bfa45f),
    (0x3fcfd3b27b368330, 0x3fef04103ff37d41),
    (0x3fd02be9ce0b87cd, 0x3feefbdeb14f4eda),
    (0x3fd06e0bfee5c057, 0x3feef389e1ea090d),
    (0x3fd0b04025245ccc, 0x3feeeb11b5442ff1),
    (0x3fd0f28696826d95, 0x3feee2760e45cb23),
    (0x3fd134dfa9805147, 0x3feed9b6cf3c4663),
    (0x3fd1774bb5687cf9, 0x3feed0d3d9d8432f),
    (0x3fd1b9cb12545e62, 0x3feec7cd0f2b5adf),
    (0x3fd1fc5e19315893, 0x3feebea24fa5d0ec),
    (0x3fd23f0523c5dc2b, 0x3feeb5537b1434da),
    (0x3fd281c08cb69be8, 0x3feeabe0709cf371),
    (0x3fd2c490af8bde81, 0x3feea2490ebdd6b8),
    (0x3fd30775e8b6eeb9, 0x3fee988d33497437),
    (0x3fd34a709597aab1, 0x3fee8eacbb648910),
    (0x3fd38d8114823369, 0x3fee84a78383435d),
    (0x3fd3d0a7c4c4bd9c, 0x3fee7a7d6766784b),
    (0x3fd413e506ad84ee, 0x3fee702e4218c668),
    (0x3fd457393b90e2aa, 0x3fee65b9edeba38e),
    (0x3fd49aa4c5cf8926, 0x3fee5b20447455cb),
    (0x3fd4de2808dce513, 0x3fee50611e88d6b5),
    (0x3fd521c36945a5f2, 0x3fee457c543ca073),
    (0x3fd565774cb66f02, 0x3fee3a71bcdd63de),
    (0x3fd5a9441a02b1fd, 0x3fee2f412eefa6f9),
    (0x3fd5ed2a392bb50f, 0x3fee23ea802b4b1a),
    (0x3fd6312a1367c57c, 0x3fee186d8577f9ec),
    (0x3fd675441329986e, 0x3fee0cca12e97895),
    (0x3fd6b978a427db95, 0x3fee00fffbbbe023),
    (0x3fd6fdc83364f719, 0x3fedf50f124fba75),
    (0x3fd742332f3702b4, 0x3fede8f7282602ae),
    (0x3fd786ba074fef93, 0x3feddcb80ddc085b),
    (0x3fd7cb5d2cc5e8ed, 0x3fedd05193273445),
    (0x3fd8101d121bed2d, 0x3fedc3c386d0ae09),
    (0x3fd854fa2b4aa1a3, 0x3fedb70db6b0e156),
    (0x3fd899f4edc962d3, 0x3fedaa2fefaae1d8),
    (0x3fd8df0dd0979384, 0x3fed9d29fda7ac9c),
    (0x3fd924454c462cc4, 0x3fed8ffbab9145d5),
    (0x3fd9699bdb019139, 0x3fed82a4c34db1c6),
    (0x3fd9af11f89ba61c, 0x3fed75250db9c792),
    (0x3fd9f4a82296347b, 0x3fed677c52a3dc9b),
    (0x3fda3a5ed82d9537, 0x3fed59aa58c6471c),
    (0x3fda80369a63aaa8, 0x3fed4baee5c1b694),
    (0x3fdac62fec0b2a92, 0x3fed3d89be176072),
    (0x3fdb0c4b51d33b86, 0x3fed2f3aa522ff95),
    (0x3fdb5289525368ab, 0x3fed20c15d14a4e5),
    (0x3fdb98ea7617ef3a, 0x3fed121da6ea5769),
    (0x3fdbdf6f47ae6904, 0x3fed034f42698214),
    (0x3fdc261853b2d785, 0x3fecf455ee182d6d),
    (0x3fdc6ce628dd132c, 0x3fece5316736032e),
    (0x3fdcb3d9580ea2b8, 0x3fecd5e169b519d7),
    (0x3fdcfaf27460fe9f, 0x3fecc665b0328622),
    (0x3fdd4232133444ad, 0x3fecb6bdf3eeb01f),
    (0x3fdd8998cc3e6049, 0x3feca6e9ecc569b9),
    (0x3fddd127399aabe7, 0x3fec96e95125c43e),
    (0x3fde18ddf7da106b, 0x3fec86bbd609a260),
    (0x3fde60bda613a78f, 0x3fec76612eed0424),
    (0x3fdea8c6e5f5e67f, 0x3fec65d90dc509f4),
    (0x3fdef0fa5bd85625, 0x3fec552322f6abf5),
    (0x3fdf3958aecddef4, 0x3fec443f1d4d22af),
    (0x3fdf81e288b7ae20, 0x3fec332ca9effdd0),
    (0x3fdfca989658baaf, 0x3fec21eb7458e5cc),
    (0x3fe009bdc3b4f877, 0x3fec107b264904d9),
    (0x3fe02e46075785a1, 0x3febfedb67be13b3),
    (0x3fe052e571060fd4, 0x3febed0bdee7064d),
    (0x3fe0779c5d4df4b8, 0x3febdb0c30185485),
    (0x3fe09c6b2a636bb7, 0x3febc8dbfdbfda88),
    (0x3fe0c152382d7366, 0x3febb67ae8584caa),
    (0x3fe0e651e85229ce, 0x3feba3e88e5c39ec),
    (0x3fe10b6a9e43942f, 0x3feb91248c38986b),
    (0x3fe1309cbf4cdb24, 0x3feb7e2e7c3ed68e),
    (0x3fe155e8b2a00052, 0x3feb6b05f6966b9b),
    (0x3fe17b4ee1641318, 0x3feb57aa912de205),
    (0x3fe1a0cfb6c3e9eb, 0x3feb441bdfab5580),
    (0x3fe1c66b9ffd666d, 0x3feb3059735c5e90),
    (0x3fe1ec230c714a96, 0x3feb1c62db2564ff),
    (0x3fe211f66db3a5a1, 0x3feb0837a370523a),
    (0x3fe237e6379cdfc6, 0x3feaf3d7561a9c44),
    (0x3fe25df2e05b6c41, 0x3feadf417a62a16d),
    (0x3fe2841ce0862974, 0x3feaca7594d44cbd),
    (0x3fe2aa64b32f7783, 0x3feab5732734fa47),
    (0x3fe2d0cad5f90e20, 0x3feaa039b06e926d),
    (0x3fe2f74fc9289adc, 0x3fea8ac8ac79d249),
    (0x3fe31df40fbd31cd, 0x3fea751f9447b724),
    (0x3fe344b82f859adf, 0x3fea5f3dddaa0225),
    (0x3fe36b9cb13786e1, 0x3fea4922fb3ac8c2),
    (0x3fe392a22087b7e9, 0x3fea32ce5c4305ef),
    (0x3fe3b9c90c43296d, 0x3fea1c3f6ca01f29),
    (0x3fe3e11206694523, 0x3fea057594a84fc8),
    (0x3fe4087da4473296, 0x3fe9ee70390dec3c),
    (0x3fe4300c7e945024, 0x3fe9d72ebac16dd6),
    (0x3fe457bf318fe517, 0x3fe9bfb076d236eb),
    (0x3fe47f965d201d78, 0x3fe9a7f4c64dfe15),
    (0x3fe4a792a4f26152, 0x3fe98ffafe1ece30),
    (0x3fe4cfb4b09d1a3e, 0x3fe977c26ee7878a),
    (0x3fe4f7fd2bc2fb34, 0x3fe95f4a64decda8),
    (0x3fe5206cc637e012, 0x3fe9469227a84b4c),
    (0x3fe5490434275b92, 0x3fe92d98fa2c355d),
    (0x3fe571c42e3d0be7, 0x3fe9145e1a6cf381),
    (0x3fe59aad71ced00f, 0x3fe8fae0c15ad38a),
    (0x3fe5c3c0c108f95c, 0x3fe8e12022a5ab3a),
    (0x3fe5ecfee31c96e7, 0x3fe8c71b6c8c49b4),
    (0x3fe61668a46ffa82, 0x3fe8acd1c7a997f1),
    (0x3fe63ffed6d198f6, 0x3fe8924256bf4545),
    (0x3fe669c251ad69e7, 0x3fe8776c367dda86),
    (0x3fe693b3f244ee17, 0x3fe85c4e7d4a0bb1),
    (0x3fe6bdd49bea05ce, 0x3fe840e83aff1d1d),
    (0x3fe6e825383cc40b, 0x3fe8253878ae2e09),
    (0x3fe712a6b76c6e92, 0x3fe8093e385a3700),
    (0x3fe73d5a107bde74, 0x3fe7ecf874b086df),
    (0x3fe76840418978a7, 0x3fe7d06620bd8524),
    (0x3fe7935a501afa78, 0x3fe7b386279d7bf3),
    (0x3fe7bea9496d5a54, 0x3fe796576c292765),
    (0x3fe7ea2e42c9027a, 0x3fe778d8c89dc27c),
    (0x3fe815ea59dab0a2, 0x3fe75b090e40447a),
    (0x3fe841deb5114bb4, 0x3fe73ce704fb7b23),
    (0x3fe86e0c84010764, 0x3fe71e716af8a794),
    (0x3fe89a74ffcc34a4, 0x3fe6ffa6f4323c0d),
    (0x3fe8c7196b9225de, 0x3fe6e0864a0050d6),
    (0x3fe8f3fb14e496b4, 0x3fe6c10e0a9e5d66),
    (0x3fe9211b54441083, 0x3fe6a13cc8a9b946),
    (0x3fe94e7b8da3cf7a, 0x3fe681110a985d4d),
    (0x3fe97c1d30f5b7d2, 0x3fe660894a2751cc),
    (0x3fe9aa01babef75e, 0x3fe63fa3f3c02962),
    (0x3fe9d82ab4b5fdfd, 0x3fe61e5f65d4d978),
    (0x3fea0699b66a8718, 0x3fe5fcb9f031317b),
    (0x3fea355065f87fa4, 0x3fe5dab1d341202a),
    (0x3fea645078c6a78c, 0x3fe5b8453f4ae294),
    (0x3fea939bb451e2a0, 0x3fe59572539c229b),
    (0x3feac333ef06451a, 0x3fe572371da8f26e),
    (0x3feaf31b1127022e, 0x3fe54e91981b7778),
    (0x3feb235315c680dc, 0x3fe52a7fa9d2f8ea),
    (0x3feb53de0bcffc24, 0x3fe505ff24d0e46b),
    (0x3feb84be172438ef, 0x3fe4e10dc51235a7),
    (0x3febb5f571cb0571, 0x3fe4bba92f53830a),
    (0x3febe7866d3b6480, 0x3fe495ceefbdc28b),
    (0x3fec197373bc7bf1, 0x3fe46f7c7879a3bc),
    (0x3fec4bbf09e19830, 0x3fe448af2027201c),
    (0x3fec7e6bd023da76, 0x3fe4216420369e50),
    (0x3fecb17c849c7288, 0x3fe3f9989320b7f7),
    (0x3fece4f404e29b3a, 0x3fe3d14972795a11),
    (0x3fed18d55010f295, 0x3fe3a87394da947a),
    (0x3fed4d2388f6360b, 0x3fe37f13aba2fbb6),
    (0x3fed81e1f875ea8d, 0x3fe355264082fea0),
    (0x3fedb714101e0a0e, 0x3fe32aa7b2d3fd8f),
    (0x3fedecbd6cf77786, 0x3fe2ff9434b34639),
    (0x3fee22e1da97bb17, 0x3fe2d3e7c7da540a),
    (0x3fee5985567b665d, 0x3fe2a79e3a2cd2e6),
    (0x3fee90ac13b18234, 0x3fe27ab321f3dcf1),
    (0x3feec85a7ee191da, 0x3fe24d21d9bcbd2d),
    (0x3fef009542b712f2, 0x3fe21ee57bd01fd8),
    (0x3fef39614cbef7d4, 0x3fe1eff8dd34fde3),
    (0x3fef72c3d2c5752a, 0x3fe1c0568830ae9d),
    (0x3fefacc258c4aaf9, 0x3fe18ff8b63353dc),
    (0x3fefe762b77744d5, 0x3fe15ed9491d3888),
    (0x3ff0115591d29d12, 0x3fe12cf1c3c6a214),
    (0x3ff02f511b223c0f, 0x3fe0fa3b41afe8ac),
    (0x3ff04da77ac5c9e5, 0x3fe0c6ae6dbb479d),
    (0x3ff06c5c6f8ce9cc, 0x3fe0924377cc95c0),
    (0x3ff08b73f9af1058, 0x3fe05cf20924c255),
    (0x3ff0aaf2613700b3, 0x3fe026b137474aca),
    (0x3ff0cadc3d4378b1, 0x3fdfdeeeea5d2083),
    (0x3ff0eb367c3fd618, 0x3fdf6e75051127be),
    (0x3ff10c066d3e6932, 0x3fdefbdeb14f4ed9),
    (0x3ff12d51caa6b58b, 0x3fde87142982b1b2),
    (0x3ff14f1ec67484ed, 0x3fde0ffbbe00c34f),
    (0x3ff1717418520341, 0x3fdd96799ba6f66c),
    (0x3ff194590de7e7f6, 0x3fdd1a6f89821640),
    (0x3ff1b7d59dd40ba2, 0x3fdc9bbc9bb903fc),
    (0x3ff1dbf27dd2221a, 0x3fdc1a3cd9862dfc),
    (0x3ff200b93cc5a540, 0x3fdb95c8d37c8daa),
    (0x3ff2263461820ad8, 0x3fdb0e35269b38f4),
    (0x3ff24c6f8f6affb2, 0x3fda8351e7be222b),
    (0x3ff27377b2570a1e, 0x3fd9f4e9f1b5859e),
    (0x3ff29b5b338b7c8d, 0x3fd962c20e982fea),
    (0x3ff2c42a3a3c7a87, 0x3fd8cc97f2912db7),
    (0x3ff2edf6fac7f5b8, 0x3fd83220fb33564e),
    (0x3ff318d619008ed9, 0x3fd79308a1dd964b),
    (0x3ff344df237486d7, 0x3fd6eeee8953be07),
    (0x3ff3722d2feb24c8, 0x3fd645640568c1c3),
    (0x3ff3a0dfa4bb4aff, 0x3fd595e8ede2700c),
    (0x3ff3d11b3fc3b697, 0x3fd4dfe7790ba697),
    (0x3ff4030b73c55372, 0x3fd422aeba618155),
    (0x3ff436e4418e69c0, 0x3fd35d6b2ed1914a),
    (0x3ff46ce4c738c4ea, 0x3fd28f1c6c540fb9),
    (0x3ff4a55ae332c7a5, 0x3fd1b6867c415165),
    (0x3ff4e0a887c40a9c, 0x3fd0d21c6aeb714f),
    (0x3ff51f4bd13f8591, 0x3fcfbfbf7ebc755d),
    (0x3ff561ebd9c18cd3, 0x3fcdba59d10bfe0a),
    (0x3ff5a96e34bc532b, 0x3fcb8cc9d3952a43),
    (0x3ff5f71d7ff42c8a, 0x3fc92ca4f010700e),
    (0x3ff64cf55148366e, 0x3fc689f26c6b01d5),
    (0x3ff6ae4c63222736, 0x3fc389d6226c1299),
    (0x3ff721a5d8718655, 0x3fbfeffbfdfebf21),
    (0x3ff7b7d33b928c5b, 0x3fb69af589b35960),
    (0x3ff921fb54442d18, 0x3c91a62633145c07),
];

pub fn fast_inv_sqrt(x: f64) -> f64 {
    let half = 0.5 * x;
    let i = 6910469410427058090i64.wrapping_sub((x.to_bits() as i64) >> 1);
    let x = f64::from_bits(i as u64);
    x * (1.5 - half * x * x)
}

pub fn atan2(mut y: f64, mut x: f64) -> f64 {
    let d2 = x * x + y * y;
    if d2.is_nan() {
        return f64::NAN;
    }
    let neg_y = y < 0.0;
    if neg_y {
        y = -y;
    }
    let neg_x = x < 0.0;
    if neg_x {
        x = -x;
    }
    let steep = y > x;
    if steep {
        std::mem::swap(&mut x, &mut y);
    }
    let rinv = fast_inv_sqrt(d2);
    x *= rinv;
    y *= rinv;
    let frac_bias = f64::from_bits(4805340802404319232);
    let yp = frac_bias + y;
    let index = yp.to_bits() as i32 as usize;
    let (phi_bits, c_phi_bits) = ASIN_COS_TAB_BITS[index];
    let (phi, c_phi) = (f64::from_bits(phi_bits), f64::from_bits(c_phi_bits));
    let s_phi = yp - frac_bias;
    let sd = y * c_phi - x * s_phi;
    let d = (6.0 + sd * sd) * sd * 0.16666666666666666;
    let mut theta = phi + d;
    if steep {
        theta = std::f64::consts::FRAC_PI_2 - theta;
    }
    if neg_x {
        theta = std::f64::consts::PI - theta;
    }
    if neg_y {
        theta = -theta;
    }
    theta
}

pub fn length_squared(x: f64, y: f64, z: f64) -> f64 {
    x * x + y * y + z * z
}

pub fn length(x: f64, y: f64, z: f64) -> f64 {
    length_squared(x, y, z).sqrt()
}

pub fn equal(a: f64, b: f64) -> bool {
    (b - a).abs() < 1.0e-5f32 as f64
}

pub fn smallest_encompassing_power_of_two(input: i32) -> i32 {
    let mut r = input - 1;
    r |= r >> 1;
    r |= r >> 2;
    r |= r >> 4;
    r |= r >> 8;
    r |= r >> 16;
    r + 1
}

pub fn is_power_of_two(input: i32) -> bool {
    input != 0 && input & (input - 1) == 0
}

pub fn ceillog2(input: i32) -> i32 {
    const DE_BRUIJN: [i32; 32] = [0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6, 11, 5, 10, 9];
    let input = if is_power_of_two(input) { input } else { smallest_encompassing_power_of_two(input) };
    DE_BRUIJN[((input as i64 * 125613361 >> 27) & 31) as usize]
}

pub fn log2(input: i32) -> i32 {
    ceillog2(input) - if is_power_of_two(input) { 0 } else { 1 }
}

pub fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut seed = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ y as i64;
    seed = seed.wrapping_mul(seed).wrapping_mul(42317861).wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

pub fn next_int(random: &mut impl RandomSource, min: i32, max: i32) -> i32 {
    if min >= max { min } else { random.next_int_bounded(max - min + 1) + min }
}

pub fn next_float(random: &mut impl RandomSource, min: f32, max: f32) -> f32 {
    if min >= max { min } else { random.next_float() * (max - min) + min }
}

pub fn next_double(random: &mut impl RandomSource, min: f64, max: f64) -> f64 {
    if min >= max { min } else { random.next_double() * (max - min) + min }
}

pub fn random_between_inclusive(random: &mut impl RandomSource, min: i32, max_inclusive: i32) -> i32 {
    random.next_int_bounded(max_inclusive - min + 1) + min
}

pub fn random_between(random: &mut impl RandomSource, min: f32, max_exclusive: f32) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

pub fn inverse_lerp(value: f64, min: f64, max: f64) -> f64 {
    (value - min) / (max - min)
}

pub fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    clamped_lerp(inverse_lerp(value, from_min, from_max), to_min, to_max)
}

pub fn clamped_lerp_f(factor: f32, min: f32, max: f32) -> f32 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp_f(factor, min, max)
    }
}

pub fn inverse_lerp_f(value: f32, min: f32, max: f32) -> f32 {
    (value - min) / (max - min)
}

pub fn clamped_map_f(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    clamped_lerp_f(inverse_lerp_f(value, from_min, from_max), to_min, to_max)
}

pub fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    lerp(inverse_lerp(value, from_min, from_max), to_min, to_max)
}

pub fn quantize(value: f64, resolution: i32) -> i32 {
    floor(value / resolution as f64) * resolution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_seed_overflows_like_java() {
        assert_eq!(get_seed(0, 0, 0), 0);
        assert_eq!(get_seed(1, 2, 3), -33674130277896);
        assert_eq!(get_seed(-30000000, 64, 30000000), -35216720112214);
    }

    #[test]
    fn table_sine() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(sin(1.0), 0.8414514);
        assert_eq!(cos(2.5), -0.80108935);
        assert_eq!(wrap_degrees(-725.5), -5.5);
    }

    #[test]
    fn logs() {
        assert_eq!(ceillog2(1), 0);
        assert_eq!(ceillog2(5), 3);
        assert_eq!(log2(30000000), 24);
        assert_eq!(smallest_encompassing_power_of_two(30000000), 33554432);
    }

    #[test]
    fn table_atan2() {
        assert_eq!(atan2(0.0, 5.0), 0.0);
        assert_eq!(atan2(3.0, -2.0), 2.1587989282661573);
        assert_eq!(atan2(-1.0, 7.0), -0.14189668178982134);
    }
}
