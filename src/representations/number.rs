use std::cmp::Ordering;

use bytes::{Buf, BufMut};
use rug::{ops::Pow, Integer, Rational};

use crate::{
    rings::{finite_field::FiniteFieldElement, Ring},
    state::{FiniteFieldIndex, State},
    utils,
};

const U8_NUM: u8 = 0b00000001;
const U16_NUM: u8 = 0b00000010;
const U32_NUM: u8 = 0b00000011;
const U64_NUM: u8 = 0b00000100;
const FIN_NUM: u8 = 0b00000101;
const ARB_NUM: u8 = 0b00000111;
const U8_DEN: u8 = 0b00010000;
const U16_DEN: u8 = 0b00100000;
const U32_DEN: u8 = 0b00110000;
const U64_DEN: u8 = 0b01000000;
const ARB_DEN: u8 = 0b01110000;
const NUM_MASK: u8 = 0b00001111;
const DEN_MASK: u8 = 0b01110000;
const SIGN: u8 = 0b10000000;

#[inline(always)]
fn get_size_of_natural(num_type: u8) -> u8 {
    match num_type {
        0 => 0,
        U8_NUM => 1,
        U16_NUM => 2,
        U32_NUM => 4,
        U64_NUM => 8,
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Number {
    Natural(i64, i64),
    Large(Rational),
    FiniteField(FiniteFieldElement<u64>, FiniteFieldIndex),
}

impl Number {
    pub fn to_borrowed<'a>(&'a self) -> BorrowedNumber<'a> {
        match self {
            Number::Natural(num, den) => BorrowedNumber::Natural(*num, *den),
            Number::Large(r) => BorrowedNumber::Large(r),
            Number::FiniteField(num, field) => BorrowedNumber::FiniteField(*num, *field),
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Number::Natural(num, _den) => *num == 0,
            Number::Large(_r) => false,
            Number::FiniteField(num, _field) => num.0 == 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedNumber<'a> {
    Natural(i64, i64),
    Large(&'a Rational),
    FiniteField(FiniteFieldElement<u64>, FiniteFieldIndex),
}

impl BorrowedNumber<'_> {
    pub fn normalize(&self) -> Number {
        match self {
            BorrowedNumber::Natural(num, den) => {
                let gcd = utils::gcd_signed(*num, *den);
                Number::Natural(*num / gcd, *den / gcd)
            }
            BorrowedNumber::Large(_) | BorrowedNumber::FiniteField(_, _) => self.to_owned(),
        }
    }

    pub fn to_owned(&self) -> Number {
        match self {
            BorrowedNumber::Natural(num, den) => Number::Natural(*num, *den),
            BorrowedNumber::Large(r) => Number::Large((*r).clone()),
            BorrowedNumber::FiniteField(num, field) => Number::FiniteField(*num, *field),
        }
    }

    pub fn add(&self, other: &BorrowedNumber<'_>, state: &State) -> Number {
        match (self, other) {
            (BorrowedNumber::Natural(n1, d1), BorrowedNumber::Natural(n2, d2)) => {
                if let Some(lcm) = d2.checked_mul(d1 / utils::gcd_signed(*d1, *d2)) {
                    if let Some(num2) = n2.checked_mul(lcm / d2) {
                        if let Some(num1) = n1.checked_mul(lcm / d1) {
                            if let Some(num) = num1.checked_add(num2) {
                                let g = utils::gcd_signed(num, lcm);
                                return Number::Natural(num / g, lcm / g);
                            }
                        }
                    }
                }
                Number::Large(Rational::from((*n1, *d1)) + Rational::from((*n2, *d2)))
            }
            // TODO: check downcast
            (BorrowedNumber::Natural(n1, d1), BorrowedNumber::Large(r2))
            | (BorrowedNumber::Large(r2), BorrowedNumber::Natural(n1, d1)) => {
                let r1 = Rational::from((*n1, *d1));
                Number::Large(r1 + *r2)
            }
            (BorrowedNumber::Large(r1), BorrowedNumber::Large(r2)) => {
                Number::Large((*r1 + *r2).into())
            }
            (BorrowedNumber::FiniteField(n1, i1), BorrowedNumber::FiniteField(n2, i2)) => {
                if i1 != i2 {
                    panic!(
                        "Cannot add numbers from different finite fields: p1={}, p2={}",
                        state.get_finite_field(*i1).get_prime(),
                        state.get_finite_field(*i2).get_prime()
                    );
                }
                let f = state.get_finite_field(*i1);
                Number::FiniteField(f.add(n1, n2), *i1)
            }
            (BorrowedNumber::FiniteField(_, _), _) => {
                panic!("Cannot add finite field to non-finite number. Convert other number first?");
            }
            (_, BorrowedNumber::FiniteField(_, _)) => {
                panic!("Cannot add finite field to non-finite number. Convert other number first?");
            }
        }
    }

    pub fn mul(&self, other: &BorrowedNumber<'_>, state: &State) -> Number {
        match (self, other) {
            (BorrowedNumber::Natural(n1, d1), BorrowedNumber::Natural(n2, d2)) => {
                let gcd1 = utils::gcd_signed(*n1 as i64, *d2 as i64);
                let gcd2 = utils::gcd_signed(*d1 as i64, *n2 as i64);

                match (n2 / gcd2).checked_mul(n1 / gcd1) {
                    Some(nn) => match (d1 / gcd2).checked_mul(d2 / gcd1) {
                        Some(nd) => Number::Natural(nn, nd),
                        None => Number::Large(Rational::from((
                            nn,
                            Integer::from(d1 / gcd2) * Integer::from(d2 / gcd1),
                        ))),
                    },
                    None => Number::Large(Rational::from((
                        Integer::from(n1 / gcd1) * Integer::from(n2 / gcd2),
                        Integer::from(d1 / gcd2) * Integer::from(d2 / gcd1),
                    ))),
                }
            }
            // TODO: check downcast
            (BorrowedNumber::Natural(n1, d1), BorrowedNumber::Large(r2))
            | (BorrowedNumber::Large(r2), BorrowedNumber::Natural(n1, d1)) => {
                let r1 = Rational::from((*n1, *d1));
                Number::Large(r1 * *r2)
            }
            (BorrowedNumber::Large(r1), BorrowedNumber::Large(r2)) => {
                Number::Large((*r1 * *r2).into())
            }
            (BorrowedNumber::FiniteField(n1, i1), BorrowedNumber::FiniteField(n2, i2)) => {
                if i1 != i2 {
                    panic!(
                        "Cannot multiply numbers from different finite fields: p1={}, p2={}",
                        state.get_finite_field(*i1).get_prime(),
                        state.get_finite_field(*i2).get_prime()
                    );
                }
                let f = state.get_finite_field(*i1);
                Number::FiniteField(f.mul(n1, n2), *i1)
            }
            (BorrowedNumber::FiniteField(_, _), _) => {
                panic!("Cannot multiply finite field to non-finite number. Convert other number first?");
            }
            (_, BorrowedNumber::FiniteField(_, _)) => {
                panic!("Cannot multiply finite field to non-finite number. Convert other number first?");
            }
        }
    }

    pub fn pow(&self, other: &BorrowedNumber<'_>, _state: &State) -> (Number, Number) {
        match (self, other) {
            (&BorrowedNumber::Natural(mut n1, mut d1), &BorrowedNumber::Natural(mut n2, d2)) => {
                if n2 < 0 {
                    n2 = -n2;
                    (n1, d1) = (d1, n1);
                }

                if n2 < u32::MAX as i64 {
                    if let Some(pn) = n1.checked_pow(n2 as u32) {
                        if let Some(pd) = d1.checked_pow(n2 as u32) {
                            // TODO: simplify 4^(1/2)
                            return (Number::Natural(pn, pd), Number::Natural(1, d2));
                        }
                    }

                    (
                        Number::Large(Rational::from((n1, d1)).pow(n2 as u32)),
                        Number::Natural(1, d2),
                    )
                } else {
                    panic!("Power is too large: {}", n2);
                }
            }
            _ => {
                unimplemented!(
                    "Power of configuration {:?}^{:?} is not implemented",
                    self,
                    other
                );
            }
        }
    }

    pub fn cmp(&self, other: &BorrowedNumber) -> Ordering {
        match (self, other) {
            (&BorrowedNumber::Natural(n1, d1), &BorrowedNumber::Natural(n2, d2)) => {
                // TODO: improve
                if n1 < 0 && n2 > 0 {
                    return Ordering::Less;
                }
                if n1 > 0 && n2 < 0 {
                    return Ordering::Greater;
                }

                match n1.checked_mul(d2) {
                    Some(a1) => match n2.checked_mul(d1) {
                        Some(a2) => a1.cmp(&a2),
                        None => Integer::from(a1).cmp(&(Integer::from(n2) * Integer::from(d1))),
                    },
                    None => (Integer::from(n1) * Integer::from(d2))
                        .cmp(&(Integer::from(n2) * Integer::from(d1))),
                }
            }
            (BorrowedNumber::Large(n1), BorrowedNumber::Large(n2)) => n1.cmp(n2),
            (BorrowedNumber::FiniteField(n1, _), BorrowedNumber::FiniteField(n2, _)) => {
                n1.0.cmp(&n2.0)
            }
            (&BorrowedNumber::Natural(n1, d1), BorrowedNumber::Large(n2)) => {
                Rational::from((n1, d1)).cmp(&n2)
            }
            (BorrowedNumber::Large(n1), &BorrowedNumber::Natural(n2, d2)) => {
                n1.cmp(&&Rational::from((n2, d2)))
            }
            _ => unreachable!(),
        }
    }
}

impl PackedRationalNumberWriter for Number {
    fn write_packed(self, dest: &mut Vec<u8>) {
        match self {
            Number::Natural(num, den) => (num, den).write_packed(dest),
            Number::Large(r) => {
                dest.put_u8(ARB_NUM | ARB_DEN);

                // note that this is not a linear representation
                let v = std::mem::ManuallyDrop::new(r);
                let lin_buf = unsafe { utils::any_as_u8_slice(&v) };

                dest.extend(lin_buf);
            }
            Number::FiniteField(num, f) => {
                dest.put_u8(FIN_NUM);
                (num.0, f.0 as u64).write_packed(dest); // this adds an extra tag
            }
        }
    }

    fn write_packed_fixed(self, mut dest: &mut [u8]) {
        match self {
            Number::Natural(num, den) => (num, den).write_packed_fixed(dest),
            Number::Large(_) => {
                todo!("Writing large packed rational not implemented")
            }
            Number::FiniteField(num, f) => {
                dest.put_u8(FIN_NUM);
                (num.0, f.0 as u64).write_packed_fixed(dest);
            }
        }
    }

    fn get_packed_size(&self) -> u64 {
        match self {
            Number::Natural(num, den) => (*num, *den).get_packed_size(),
            Number::Large(_) => 1 + std::mem::size_of::<Rational>() as u64,
            Number::FiniteField(m, i) => 2 + (m.0, i.0 as u64).get_packed_size(),
        }
    }
}

/// A generalized rational number. The first byte indicates the sign, size and type of the numerator and denominator.
/// The highest four bits give the byte size of the numerator and the lower bits of the denominator.
pub trait PackedRationalNumberWriter {
    /// Write a single number.
    fn write_packed(self, dest: &mut Vec<u8>);
    /// Write a fraction to a fixed-size buffer.
    fn write_packed_fixed(self, dest: &mut [u8]);
    /// Get the number of bytes of the packed representation.
    fn get_packed_size(&self) -> u64;
}

/// A reader for generalized rational numbers. See [`RationalNumberWriter`].
pub trait PackedRationalNumberReader {
    fn get_number_view(&self) -> (BorrowedNumber, &[u8]);
    fn get_frac_u64(&self) -> (u64, u64, &[u8]);
    fn get_frac_i64(&self) -> (i64, i64, &[u8]);
    fn skip_rational(&self) -> &[u8];
    fn is_zero_rat(&self) -> bool;
    fn is_one_rat(&self) -> bool;
}

impl PackedRationalNumberReader for [u8] {
    #[inline(always)]
    fn get_number_view(&self) -> (BorrowedNumber, &[u8]) {
        let mut source = self;
        let disc = source.get_u8();
        if (disc & NUM_MASK) == ARB_NUM {
            let rat: &Rational = unsafe { std::mem::transmute(&source[0]) };
            (
                BorrowedNumber::Large(rat),
                &source[std::mem::size_of::<Rational>()..],
            )
        } else if (disc & NUM_MASK) == FIN_NUM {
            let (num, fi);
            (num, fi, source) = source.get_frac_u64();
            (
                BorrowedNumber::FiniteField(FiniteFieldElement(num), FiniteFieldIndex(fi as usize)),
                source,
            )
        } else {
            let (num, den, source) = self.get_frac_i64();
            (BorrowedNumber::Natural(num, den), source)
        }
    }

    #[inline(always)]
    fn get_frac_u64(&self) -> (u64, u64, &[u8]) {
        let mut source = self;
        let disc = source.get_u8();
        let num;
        (num, source) = match disc & NUM_MASK {
            U8_NUM => {
                let v = source.get_u8();
                (v as u64, source)
            }
            U16_NUM => {
                let v = source.get_u16_le();
                (v as u64, source)
            }
            U32_NUM => {
                let v = source.get_u32_le();
                (v as u64, source)
            }
            U64_NUM => {
                let v = source.get_u64_le();
                (v as u64, source)
            }
            ARB_NUM => {
                panic!("Overflow")
            }
            x => {
                unreachable!("Unsupported numerator type {}", x)
            }
        };

        let den;
        (den, source) = match disc & DEN_MASK {
            0 => (1u64, source),
            U8_DEN => {
                let v = source.get_u8();
                (v as u64, source)
            }
            U16_DEN => {
                let v = source.get_u16_le();
                (v as u64, source)
            }
            U32_DEN => {
                let v = source.get_u32_le();
                (v as u64, source)
            }
            U64_DEN => {
                let v = source.get_u64_le();
                (v as u64, source)
            }
            ARB_DEN => {
                panic!("Overflow")
            }
            x => {
                unreachable!("Unsupported denominator type {}", x)
            }
        };

        (num, den, source)
    }

    #[inline(always)]
    fn get_frac_i64(&self) -> (i64, i64, &[u8]) {
        let mut source = self;
        let disc = source.get_u8();
        let num;
        (num, source) = match disc & NUM_MASK {
            U8_NUM => {
                let v = source.get_u8();
                (v as i64, source)
            }
            U16_NUM => {
                let v = source.get_u16_le();
                (v as i64, source)
            }
            U32_NUM => {
                let v = source.get_u32_le();
                (v as i64, source)
            }
            U64_NUM => {
                let v = source.get_u64_le();
                (v as i64, source)
            }
            ARB_NUM => {
                panic!("Overflow")
            }
            x => {
                unreachable!("Unsupported numerator type {}", x)
            }
        };

        let den;
        (den, source) = match disc & DEN_MASK {
            0 => (1i64, source),
            U8_DEN => {
                let v = source.get_u8();
                (v as i64, source)
            }
            U16_DEN => {
                let v = source.get_u16_le();
                (v as i64, source)
            }
            U32_DEN => {
                let v = source.get_u32_le();
                (v as i64, source)
            }
            U64_DEN => {
                let v = source.get_u64_le();
                (v as i64, source)
            }
            ARB_DEN => {
                panic!("Overflow")
            }
            x => {
                unreachable!("Unsupported denominator type {}", x)
            }
        };

        if disc & SIGN != 0 {
            (-num, den, source)
        } else {
            (num, den, source)
        }
    }

    #[inline(always)]
    fn skip_rational(&self) -> &[u8] {
        let mut dest = self;
        let var_size = dest.get_u8();

        let v_num = var_size & NUM_MASK;
        if v_num == ARB_NUM {
            dest.advance(std::mem::size_of::<Rational>());
            dest
        } else if v_num == FIN_NUM {
            let var_size = dest.get_u8();
            let size = get_size_of_natural(var_size & NUM_MASK)
                + get_size_of_natural((var_size & DEN_MASK) >> 4);
            dest.advance(size as usize);
            dest
        } else {
            let size = get_size_of_natural(v_num) + get_size_of_natural((var_size & DEN_MASK) >> 4);
            dest.advance(size as usize);
            dest
        }
    }

    #[inline(always)]
    fn is_zero_rat(&self) -> bool {
        // TODO: make a zero have no number at all (i.e., self[1] = 0)
        self[1] == 1 && self[2] == 0
    }

    #[inline(always)]
    fn is_one_rat(&self) -> bool {
        self[1] == 1 && self[2] == 1
    }
}

impl PackedRationalNumberWriter for (i64, i64) {
    #[inline(always)]
    fn write_packed(self, dest: &mut Vec<u8>) {
        let p = dest.len();

        let num_u64 = self.0.abs() as u64;
        let den_u64 = self.1.abs() as u64;
        (num_u64, den_u64).write_packed(dest);

        if self.0 >= 0 && self.1 < 0 || self.0 < 0 && self.1 >= 0 {
            dest[p] |= SIGN;
        }
    }

    #[inline(always)]
    fn write_packed_fixed(self, dest: &mut [u8]) {
        let num_u64 = self.0.abs() as u64;
        let den_u64 = self.1.abs() as u64;
        (num_u64, den_u64).write_packed_fixed(dest);

        if self.0 >= 0 && self.1 < 0 || self.0 < 0 && self.1 >= 0 {
            dest[0] |= SIGN;
        }
    }

    fn get_packed_size(&self) -> u64 {
        (self.0 as u64, self.1 as u64).get_packed_size()
    }
}

impl PackedRationalNumberWriter for (u64, u64) {
    #[inline(always)]
    fn write_packed(self, dest: &mut Vec<u8>) {
        let p = dest.len();

        if self.0 < u8::MAX as u64 {
            dest.put_u8(U8_NUM);
            dest.put_u8(self.0 as u8);
        } else if self.0 < u16::MAX as u64 {
            dest.put_u8(U16_NUM);
            dest.put_u16_le(self.0 as u16);
        } else if self.0 < u32::MAX as u64 {
            dest.put_u8(U32_NUM);
            dest.put_u32_le(self.0 as u32);
        } else {
            dest.put_u8(U64_NUM);
            dest.put_u64_le(self.0);
        }

        if self.1 == 1 {
        } else if self.1 < u8::MAX as u64 {
            dest[p] |= U8_DEN;
            dest.put_u8(self.1 as u8);
        } else if self.1 < u16::MAX as u64 {
            dest[p] |= U16_DEN;
            dest.put_u16_le(self.1 as u16);
        } else if self.1 < u32::MAX as u64 {
            dest[p] |= U32_DEN;
            dest.put_u8(3);
            dest.put_u32_le(self.1 as u32);
        } else {
            dest[p] |= U64_DEN;
            dest.put_u64_le(self.1);
        }
    }

    #[inline(always)]
    fn write_packed_fixed(self, dest: &mut [u8]) {
        let (tag, mut dest) = dest.split_first_mut().unwrap();

        if self.0 < u8::MAX as u64 {
            *tag = U8_NUM;
            dest.put_u8(self.0 as u8);
        } else if self.0 < u16::MAX as u64 {
            *tag = U16_NUM;
            dest.put_u16_le(self.0 as u16);
        } else if self.0 < u32::MAX as u64 {
            *tag = U32_NUM;
            dest.put_u32_le(self.0 as u32);
        } else {
            *tag = U64_NUM;
            dest.put_u64_le(self.0);
        }

        if self.1 == 1 {
        } else if self.1 < u8::MAX as u64 {
            *tag |= U8_DEN;
            dest.put_u8(self.1 as u8);
        } else if self.1 < u16::MAX as u64 {
            *tag |= U16_DEN;
            dest.put_u16_le(self.1 as u16);
        } else if self.1 < u32::MAX as u64 {
            *tag |= U32_DEN;
            dest.put_u8(3);
            dest.put_u32_le(self.1 as u32);
        } else {
            *tag |= U64_DEN;
            dest.put_u64_le(self.1);
        }
    }

    fn get_packed_size(&self) -> u64 {
        let mut size = 1;
        size += if self.0 < u8::MAX as u64 {
            get_size_of_natural(U8_NUM)
        } else if self.0 < u16::MAX as u64 {
            get_size_of_natural(U16_NUM)
        } else if self.0 < u32::MAX as u64 {
            get_size_of_natural(U32_NUM)
        } else {
            get_size_of_natural(U64_NUM)
        };

        size += if self.1 == 1 {
            0
        } else if self.1 < u8::MAX as u64 {
            get_size_of_natural(U8_NUM)
        } else if self.1 < u16::MAX as u64 {
            get_size_of_natural(U16_NUM)
        } else if self.1 < u32::MAX as u64 {
            get_size_of_natural(U32_NUM)
        } else {
            get_size_of_natural(U64_NUM)
        };
        size as u64
    }
}
