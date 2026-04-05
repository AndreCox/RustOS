static mut RAND_STATE: u64 = 0x1234_abcd;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rand() -> i32 {
    unsafe {
        RAND_STATE = RAND_STATE.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((RAND_STATE >> 33) & 0x7fffffff) as i32
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn srand(seed: u32) {
    unsafe { RAND_STATE = seed as u64 };
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn sqrt(_x: f64) -> f64 {
    core::arch::naked_asm!("sqrtsd xmm0, xmm0", "ret")
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn fabs(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "movq    rax, xmm0",
        "btr     rax, 63",
        "movq    xmm0, rax",
        "ret"
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn sin(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "call    {sin_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        sin_rust = sym sin_rust
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn cos(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "call    {cos_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        cos_rust = sym cos_rust
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn atan2(_y: f64, _x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "movq    rsi, xmm1",
        "call    {atan2_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        atan2_rust = sym atan2_rust
    )
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn pow(_base: f64, _exp: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "movq    rsi, xmm1",
        "call    {pow_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        pow_rust = sym pow_rust
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn cos_rust(x_bits: u64) -> u64 {
    let x = reduce_angle(f64::from_bits(x_bits));
    sin_rust((x + HALF_PI).to_bits())
}
// ---------------------------------------------------------
// Pure Rust Math Implementations (Bitcast isolated)
// ---------------------------------------------------------

const PI: f64 = 3.141592653589793;
const TWO_PI: f64 = 6.283185307179586;
const HALF_PI: f64 = 1.5707963267948966;

fn abs_f64(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

fn reduce_angle(x: f64) -> f64 {
    // Use f64 rem_euclid equivalent without std
    let factor = (x * (1.0 / TWO_PI)) as i64;
    let mut x = x - (factor as f64) * TWO_PI;
    // Belt-and-suspenders cleanup for floating point residue
    while x > PI {
        x -= TWO_PI;
    }
    while x < -PI {
        x += TWO_PI;
    }
    x
}

#[unsafe(no_mangle)]
pub extern "C" fn sin_rust(x_bits: u64) -> u64 {
    let x = reduce_angle(f64::from_bits(x_bits));
    // Taylor for sin: x - x^3/6 + x^5/120 - x^7/5040 + x^9/362880
    let x2 = x * x;
    let out = x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))));
    out.to_bits()
}

#[unsafe(no_mangle)]
pub extern "C" fn acos_rust(x_bits: u64) -> u64 {
    let mut x = f64::from_bits(x_bits);
    if x > 1.0 {
        x = 1.0;
    }
    if x < -1.0 {
        x = -1.0;
    }
    // Approximation: acos(x) = sqrt(1-x) * (1.5707963267948966 - 0.213300989*x + 0.07786196*x*x - 0.015024462*x*x*x ... )
    // A classic robust Nvidia approximation
    let a = 1.434128;
    let out = if x == 1.0 {
        0.0
    } else if x == -1.0 {
        PI
    } else {
        let neg = x < 0.0;
        let nx = abs_f64(x);
        let mut res = unsafe { sqrt(1.0 - nx) }
            * (1.5707288 - nx * (0.2121144 - nx * (0.0742610 - nx * 0.0187293)));
        if neg { PI - res } else { res }
    };
    out.to_bits()
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn acos(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "call    {acos_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        acos_rust = sym acos_rust
    )
}

fn atan_approx(x: f64) -> f64 {
    let mut a = abs_f64(x);
    let inv = a > 1.0;
    if inv {
        a = 1.0 / a;
    }

    // Rational approx for atan in [0, 1]
    let a2 = a * a;
    let mut res = a / (1.0 + 0.28125 * a2);

    if inv {
        res = HALF_PI - res;
    }
    if x < 0.0 { -res } else { res }
}

#[unsafe(no_mangle)]
pub extern "C" fn atan2_rust(y_bits: u64, x_bits: u64) -> u64 {
    let y = f64::from_bits(y_bits);
    let x = f64::from_bits(x_bits);

    let res = if abs_f64(x) < 1e-8 {
        if y > 0.0 {
            HALF_PI
        } else if y < 0.0 {
            -HALF_PI
        } else {
            0.0
        }
    } else {
        let ratio = y / x;
        let a = atan_approx(ratio);
        if x < 0.0 {
            if y >= 0.0 { a + PI } else { a - PI }
        } else {
            a
        }
    };
    res.to_bits()
}

#[unsafe(no_mangle)]
pub extern "C" fn pow_rust(base_bits: u64, exp_bits: u64) -> u64 {
    let base = f64::from_bits(base_bits);
    let exp = f64::from_bits(exp_bits);

    if exp == 0.5 {
        return unsafe { sqrt(base) }.to_bits();
    }
    if exp == -0.5 {
        return (1.0 / unsafe { sqrt(base) }).to_bits();
    }
    if exp == 0.0 {
        return 1.0f64.to_bits();
    }
    if exp == 1.0 {
        return base_bits;
    }
    if exp == 2.0 {
        return (base * base).to_bits();
    }

    // Simplistic integer iteration for expected powers (Quake rarely uses complex pow)
    let e_i = exp as i32;
    if (e_i as f64) == exp {
        let mut r = 1.0;
        let mut b = base;
        let mut e = if e_i < 0 { -e_i } else { e_i };
        while e > 0 {
            if e % 2 == 1 {
                r *= b;
            }
            b *= b;
            e /= 2;
        }
        if e_i < 0 {
            r = 1.0 / r;
        }
        return r.to_bits();
    }

    // Fallback error value (avoiding pulling in exp/log for Quake bounds)
    1.0f64.to_bits()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn modf(x: f64, iptr: *mut f64) -> f64 {
    let abs_x = if x < 0.0 { -x } else { x };
    if abs_x >= 4503599627370496.0 || x != x {
        // 2^52, same guard as floor/ceil
        *iptr = x;
        return 0.0;
    }
    let i = (x as i64) as f64;
    *iptr = i;
    x - i
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn floor(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "call    {floor_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        floor_rust = sym floor_rust
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn floor_rust(x_bits: u64) -> u64 {
    let x = f64::from_bits(x_bits);
    let abs_x = if x < 0.0 { -x } else { x };
    if abs_x >= 4503599627370496.0 || x != x {
        return x_bits;
    }
    let mut i = x as i64;
    if (i as f64) > x {
        i -= 1;
    }
    (i as f64).to_bits()
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn ceil(_x: f64) -> f64 {
    core::arch::naked_asm!(
        "sub     rsp, 16",
        "movq    rdi, xmm0",
        "call    {ceil_rust}",
        "movq    xmm0, rax",
        "add     rsp, 16",
        "ret",
        ceil_rust = sym ceil_rust
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn ceil_rust(x_bits: u64) -> u64 {
    let x = f64::from_bits(x_bits);
    let abs_x = if x < 0.0 { -x } else { x };
    if abs_x >= 4503599627370496.0 || x != x {
        return x_bits;
    }
    let mut i = x as i64;
    if (i as f64) < x {
        i += 1;
    }
    (i as f64).to_bits()
}

#[unsafe(no_mangle)]
pub extern "C" fn abs(n: i32) -> i32 {
    if n < 0 { -n } else { n }
}
