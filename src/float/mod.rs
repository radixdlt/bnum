use crate::{ExpType, BUintD8};
use crate::bint::BIntD8;
use crate::digit::u8 as digit;
//use crate::ExpType;
use crate::cast::As;

type Digit = u8;

#[cfg(test)]
pub type F64 = Float<8, 52>;

#[cfg(test)]
pub type F32 = Float<4, 23>;

macro_rules! handle_nan {
    ($ret: expr; $($n: expr), +) => {
        if $($n.is_nan()) || + {
            return $ret;
        }
    };
}

mod cast;
mod classify;
mod cmp;
mod consts;
mod convert;
mod endian;
mod math;
mod ops;
mod to_str;

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Float<const W: usize, const MB: usize> {
    bits: BUintD8<W>,
}

// TODO: implement rand traits

impl<const W: usize, const MB: usize> Float<W, MB> {
    const MB: ExpType = MB as _;
    const BITS: ExpType = BUintD8::<W>::BITS;
    
    const EXPONENT_BITS: ExpType = Self::BITS - Self::MB - 1;

    /*const MANTISSA_WORDS: (usize, usize) = (MB / digit::BITS as usize, MB % digit::BITS as usize);

    const EXPONENT_MASK: BUintD8<W> = BUintD8::MAX.wrapping_shl(Self::MB) ^ BIntD8::MIN.to_bits();*/
}

impl<const W: usize, const MB: usize> Float<W, MB> {
    #[inline]
    pub const fn from_parts(negative: bool, exponent: BUintD8<W>, mantissa: BUintD8<W>) -> Self {
        let mut words = *(exponent | mantissa).digits();
        if negative {
            words[W - 1] |= 1 << (digit::BITS - 1);
        }
        Self::from_words(words)
    }

    #[inline(always)]
    const fn from_words(words: [Digit; W]) -> Self {
        Self::from_bits(BUintD8::from_digits(words))
    }

    #[inline(always)]
    const fn words(&self) -> &[Digit; W] {
        &self.bits.digits
    }

    #[inline]
    const fn exponent(self) -> BIntD8<W> {
        BIntD8::from_bits(self.exp_mant().0)
    }
    
    /*const fn actual_exponent(self) -> BIntD8<W> {
        self.exponent() - Self::EXP_BIAS
    }
    const fn unshifted_exponent(self) -> BIntD8<W> {
        BIntD8::from_bits(self.to_bits() & Self::EXPONENT_MASK)
    }*/
    const MANTISSA_MASK: BUintD8<W> = BUintD8::MAX.wrapping_shr(Self::EXPONENT_BITS + 1);
    /*const fn mantissa(self) -> BUintD8<W> {
        self.to_bits() & Self::MANTISSA_MASK
    }
    const fn actual_mantissa(self) -> BUintD8<W> {
        if self.is_subnormal() {
            self.mantissa()
        } else {
            self.mantissa() | (BUintD8::ONE.wrapping_shl(MB))
        }
    }*/
    #[inline(always)]
    const fn to_int(self) -> BIntD8<W> {
        BIntD8::from_bits(self.to_bits())
    }

    #[inline]
    pub const fn copysign(self, sign: Self) -> Self {
        let mut self_words = *self.words();
        if sign.is_sign_negative() {
            self_words[W - 1] |= 1 << (digit::BITS - 1);
        } else {
            self_words[W - 1] &= (!0) >> 1;
        }
        Self::from_bits(BUintD8::from_digits(self_words))
    }

    #[inline]
    pub const fn signum(self) -> Self {
        handle_nan!(Self::NAN; self);
        Self::ONE.copysign(self)
    }

	#[inline]
	pub const fn next_up(self) -> Self {
		use core::num::FpCategory;

		match self.classify() {
			FpCategory::Nan => self,
			FpCategory::Infinite => if self.is_sign_negative() {
				Self::MIN
			} else {
				self
			},
			FpCategory::Zero => Self::MIN_POSITIVE_SUBNORMAL,
			_ => if self.is_sign_negative() {
				Self::from_bits(self.to_bits() - BUintD8::ONE)
			} else {
				Self::from_bits(self.to_bits() + BUintD8::ONE)
			},
		}
	}

	#[inline]
	pub const fn next_down(self) -> Self {
		use core::num::FpCategory;

		match self.classify() {
			FpCategory::Nan => self,
			FpCategory::Infinite => if self.is_sign_negative() {
				self
			} else {
				Self::MAX
			},
			FpCategory::Zero => Self::MAX_NEGATIVE_SUBNORMAL,
			_ => if self.is_sign_negative() {
				Self::from_bits(self.to_bits() + BUintD8::ONE)
			} else {
				Self::from_bits(self.to_bits() - BUintD8::ONE)
			}
		}
	}
}

impl<const W: usize, const MB: usize> Default for Float<W, MB> {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl<const W: usize, const MB: usize> Float<W, MB> {
    #[inline]
    const fn exp_mant(&self) -> (BUintD8<W>, BUintD8<W>) {
        let bits = self.bits;
        let exp = (bits << 1u8) >> (Self::MB + 1);
        let mant = bits & Self::MANTISSA_MASK;
        
        if exp.is_zero() {
            (BUintD8::ONE, mant)
        } else {
            (exp, mant | (BUintD8::ONE << Self::MB))
        }
    }

    #[inline]
    pub(super) const fn decode(self) -> (BUintD8<W>, BIntD8<W>) {
        let bits = self.bits;
        let exp = (bits << 1u8) >> (Self::MB + 1);
        let mant = if exp.is_zero() {
            (bits & Self::MANTISSA_MASK) << 1 as ExpType
        } else {
            (bits & Self::MANTISSA_MASK) | (BUintD8::power_of_two(MB as ExpType))
        };
        let exp = BIntD8::from_bits(exp) - Self::EXP_BIAS + MB.as_::<BIntD8<W>>();
        (mant, exp)
    }

    #[inline]
    const fn from_exp_mant(negative: bool, exp: BUintD8<W>, mant: BUintD8<W>) -> Self {
        let mut bits = (exp << Self::MB) | mant;
        if negative {
            bits = bits | BIntD8::MIN.to_bits();
        }
        let f = Self::from_bits(bits);
        f
    }
}

#[cfg(test)]
impl From<f64> for F64 {
    #[inline]
    fn from(f: f64) -> Self {
        Self::from_bits(f.to_bits().into())
    }
}

#[cfg(test)]
impl From<f32> for F32 {
    #[inline]
    fn from(f: f32) -> Self {
        Self::from_bits(f.to_bits().into())
    }
}

#[cfg(test)]
mod tests {
	use crate::test::test_bignum;
	use crate::test::types::{ftest, FTEST};

    test_bignum! {
        function: <ftest>::copysign(f1: ftest, f2: ftest)
    }

    test_bignum! {
        function: <ftest>::signum(f: ftest)
    }

    test_bignum! {
		function: <ftest>::next_up(f: ftest)
	}

    test_bignum! {
		function: <ftest>::next_down(f: ftest)
	}
}