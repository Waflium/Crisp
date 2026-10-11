#[derive(Clone, Copy, Debug)]
pub struct Fraction {
    numerator: i32,
    denominator: i32,
}

pub type ArithmeticResult<T> = Option<T>;

fn greatest_common_divisor(u: i32, v: i32) -> ArithmeticResult<i32> {
    if u == 0 || v == 0 {
        if u == i32::MIN || v == i32::MIN {
            return None;
        }
        return Some(u.abs() + v.abs());
    }
    let (mut a, mut b) = ((u as i64).abs(), (v as i64).abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    i32::try_from(a).ok()
}

fn mul_and_check(x: i32, y: i32) -> ArithmeticResult<i32> {
    x.checked_mul(y)
}

fn mul_pos_and_check(x: i32, y: i32) -> ArithmeticResult<i32> {
    x.checked_mul(y)
}

impl Fraction {
    pub const ZERO: Fraction = Fraction { numerator: 0, denominator: 1 };
    pub const ONE: Fraction = Fraction { numerator: 1, denominator: 1 };

    pub const fn get_fraction(mut numerator: i32, mut denominator: i32) -> ArithmeticResult<Fraction> {
        if denominator == 0 {
            return None;
        }
        if denominator < 0 {
            if numerator == i32::MIN || denominator == i32::MIN {
                return None;
            }
            numerator = -numerator;
            denominator = -denominator;
        }
        Some(Fraction { numerator, denominator })
    }

    pub fn get_reduced_fraction(mut numerator: i32, mut denominator: i32) -> ArithmeticResult<Fraction> {
        if denominator == 0 {
            return None;
        }
        if numerator == 0 {
            return Some(Fraction::ZERO);
        }
        if denominator == i32::MIN && numerator & 1 == 0 {
            numerator /= 2;
            denominator /= 2;
        }
        if denominator < 0 {
            if numerator == i32::MIN || denominator == i32::MIN {
                return None;
            }
            numerator = -numerator;
            denominator = -denominator;
        }
        let gcd = greatest_common_divisor(numerator, denominator)?;
        Some(Fraction { numerator: numerator / gcd, denominator: denominator / gcd })
    }

    pub fn int_value(self) -> i32 {
        self.numerator / self.denominator
    }

    fn negate(self) -> ArithmeticResult<Fraction> {
        if self.numerator == i32::MIN {
            return None;
        }
        Some(Fraction { numerator: -self.numerator, denominator: self.denominator })
    }

    fn invert(self) -> ArithmeticResult<Fraction> {
        if self.numerator == 0 || self.numerator == i32::MIN {
            return None;
        }
        if self.numerator < 0 {
            Some(Fraction { numerator: -self.denominator, denominator: -self.numerator })
        } else {
            Some(Fraction { numerator: self.denominator, denominator: self.numerator })
        }
    }

    pub fn add(self, fraction: Fraction) -> ArithmeticResult<Fraction> {
        self.add_sub(fraction, true)
    }

    pub fn subtract(self, fraction: Fraction) -> ArithmeticResult<Fraction> {
        self.add_sub(fraction, false)
    }

    fn add_sub(self, fraction: Fraction, is_add: bool) -> ArithmeticResult<Fraction> {
        if self.numerator == 0 {
            return if is_add { Some(fraction) } else { fraction.negate() };
        }
        if fraction.numerator == 0 {
            return Some(self);
        }
        let d1 = greatest_common_divisor(self.denominator, fraction.denominator)?;
        if d1 == 1 {
            let uvp = mul_and_check(self.numerator, fraction.denominator)?;
            let upv = mul_and_check(fraction.numerator, self.denominator)?;
            let numerator = if is_add { uvp.checked_add(upv)? } else { uvp.checked_sub(upv)? };
            return Some(Fraction { numerator, denominator: mul_pos_and_check(self.denominator, fraction.denominator)? });
        }
        let uvp = self.numerator as i128 * (fraction.denominator / d1) as i128;
        let upv = fraction.numerator as i128 * (self.denominator / d1) as i128;
        let t = if is_add { uvp + upv } else { uvp - upv };
        let tmodd1 = t.rem_euclid(d1 as i128) as i32;
        let d2 = if tmodd1 == 0 { d1 } else { greatest_common_divisor(tmodd1, d1)? };
        let w = t / d2 as i128;
        let numerator = i32::try_from(w).ok()?;
        Some(Fraction { numerator, denominator: mul_pos_and_check(self.denominator / d1, fraction.denominator / d2)? })
    }

    pub fn multiply_by(self, fraction: Fraction) -> ArithmeticResult<Fraction> {
        if self.numerator == 0 || fraction.numerator == 0 {
            return Some(Fraction::ZERO);
        }
        let d1 = greatest_common_divisor(self.numerator, fraction.denominator)?;
        let d2 = greatest_common_divisor(fraction.numerator, self.denominator)?;
        Fraction::get_reduced_fraction(
            mul_and_check(self.numerator / d1, fraction.numerator / d2)?,
            mul_pos_and_check(self.denominator / d2, fraction.denominator / d1)?,
        )
    }

    pub fn divide_by(self, fraction: Fraction) -> ArithmeticResult<Fraction> {
        if fraction.numerator == 0 {
            return None;
        }
        self.multiply_by(fraction.invert()?)
    }
}
