// Given
// M31 a = a_1 + (a_2 << 8) + (a_3 << 16) + (a_4 << 24) where a_1, a_2, a_3 are 8-bit and a_4 is 7-bit.
// M31 b = b_1 + (b_2 << 8) + (b_3 << 16) + (b_4 << 24) similarly.
//
// a * b
// = c_1 + (c_2 << 8) + (c_3 << 16) + (c_4 << 24) + (c_5 << 32) + (c_6 << 40) + (c_7 << 48)
// where c_1...c_6 are 16-bit and c_7 is at most 14-bit
//
// c_1 = (a_1 * b_1) + (a_2 * b_4 + a_3 * b_3 + a_4 * b_2) << 1
// c_2 = (a_1 * b_2 + a_2 * b_1) + (a_3 * b_4 + a_4 * b_3) << 1
// c_3 = (a_1 * b_3 + a_2 * b_2 + a_3 * b_1) + (a_4 * b_4) << 1
// c_4 = a_1 * b_4 + a_2 * b_3 + a_3 * b_2 + a_4 * b_1
//
// in total 16 8-bit mult
//
// Now, reduction phase
//
// given hints: q
// Compute q * (1 << 31 - 1) = q * (1 << 31) - q
//
//   q * (1 << 31)
// = (q << 7) << 24
//
// now do the following:
// - t = c_4
// - t -= q << 7
// - t <<= 8
// - t += c_3
// - t <<= 8
// - t += c_2
// - t <<= 8
// - t += q
// - t += c_1
// - r = t

use crate::lookup::Lookup8BitGadget;
use crate::utils::m31_to_limbs;
use anyhow::{Error, Result};
use bitcoin_circle_stark::treepp::*;
use bitcoin_circle_stark::OP_HINT;

pub struct M31Mult;

impl M31Mult {
    pub fn compute_c_limbs(pairs: &[(u32, u32)]) -> Result<[i32; 4]> {
        let mut c_limbs = [0i32; 4];

        for &(a, b) in pairs.iter() {
            let a_limbs = m31_to_limbs(a);
            let b_limbs = m31_to_limbs(b);

            c_limbs[0] += i32::try_from(a_limbs[0] * b_limbs[0])?;

            c_limbs[1] += i32::try_from(a_limbs[0] * b_limbs[1])?;
            c_limbs[1] += i32::try_from(a_limbs[1] * b_limbs[0])?;

            c_limbs[2] += i32::try_from(a_limbs[0] * b_limbs[2])?;
            c_limbs[2] += i32::try_from(a_limbs[1] * b_limbs[1])?;
            c_limbs[2] += i32::try_from(a_limbs[2] * b_limbs[0])?;

            c_limbs[3] += i32::try_from(a_limbs[0] * b_limbs[3])?;
            c_limbs[3] += i32::try_from(a_limbs[1] * b_limbs[2])?;
            c_limbs[3] += i32::try_from(a_limbs[2] * b_limbs[1])?;
            c_limbs[3] += i32::try_from(a_limbs[3] * b_limbs[0])?;

            c_limbs[0] += i32::try_from(a_limbs[1] * b_limbs[3])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;
            c_limbs[0] += i32::try_from(a_limbs[2] * b_limbs[2])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;
            c_limbs[0] += i32::try_from(a_limbs[3] * b_limbs[1])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;

            c_limbs[1] += i32::try_from(a_limbs[2] * b_limbs[3])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;
            c_limbs[1] += i32::try_from(a_limbs[3] * b_limbs[2])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;

            c_limbs[2] += i32::try_from(a_limbs[3] * b_limbs[3])?
                .checked_shl(1)
                .ok_or(Error::msg("Unexpected overflow"))?;
        }

        Ok(c_limbs)
    }

    pub fn compute_q(c_limbs: &[i32; 4]) -> Result<i64> {
        let mut sum = 0i64;
        sum = sum
            .checked_add(c_limbs[3] as i64)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_shl(8)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_add(c_limbs[2] as i64)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_shl(8)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_add(c_limbs[1] as i64)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_shl(8)
            .ok_or(Error::msg("Unexpected overflow"))?;
        sum = sum
            .checked_add(c_limbs[0] as i64)
            .ok_or(Error::msg("Unexpected overflow"))?;

        let q = sum / ((1 << 31) - 1);
        Ok(q)
    }
}

pub struct M31MultGadget;

impl M31MultGadget {
    // Compute c from a, b.
    //
    // Input:
    // - table
    // - (k elements)
    // - a1, a2, a3, a4
    // - b1, b2, b3, b4
    //
    // Output:
    // - table
    // - (k elements)
    // - c4, c3, c2, c1
    pub fn compute_c_limbs(k: usize) -> Script {
        script! {
            // c_1 = a1 * b1
            { 7 + 0 + 0 } OP_PICK
            { 3 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 0) }
            OP_TOALTSTACK

            // c_2 = a1 * b2 + a2 * b1
            { 7 + 0 + 0 } OP_PICK
            { 2 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 0) }
            { 6 + 0 + 1 } OP_PICK
            { 3 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 1) }
            OP_ADD OP_TOALTSTACK

            // c_3 = a_1 * b_3 + a_2 * b_2 + a_3 * b_1
            { 7 + 0 + 0 } OP_PICK
            { 1 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 0) }
            { 6 + 0 + 1 } OP_PICK
            { 2 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 1) }
            OP_ADD
            { 5 + 0 + 1 } OP_PICK
            { 3 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 8 + 1) }
            OP_ADD OP_TOALTSTACK

            // c_4 = a_1 * b_4 + a_2 * b_3 + a_3 * b_2 + a_4 * b_1
            { 7 + 0 + 0 } OP_ROLL
            { 0 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 7 + 0) }
            { 6 + 0 + 1 } OP_PICK
            { 1 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 7 + 1) }
            OP_ADD
            { 5 + 0 + 1 } OP_PICK
            { 2 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 7 + 1) }
            OP_ADD
            { 4 + 0 + 1 } OP_PICK
            { 3 + 1 + 1 } OP_ROLL
            { Lookup8BitGadget::lookup(k + 6 + 1) }
            OP_ADD OP_TOALTSTACK

            // - table
            // - (k elements)
            // - a2, a3, a4
            // - b2, b3, b4

            // c_5 = a_2 * b_4 + a_3 * b_3 + a_4 * b_2
            { 5 + 0 + 0 } OP_ROLL
            { 0 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 5 + 0) }
            { 4 + 0 + 1 } OP_PICK
            { 1 + 1 + 1 } OP_PICK
            { Lookup8BitGadget::lookup(k + 5 + 1) }
            OP_ADD
            { 3 + 0 + 1 } OP_PICK
            { 2 + 1 + 1 } OP_ROLL
            { Lookup8BitGadget::lookup(k + 4 + 1) }
            OP_ADD OP_TOALTSTACK

            // - table
            // - (k elements)
            // - a3, a4
            // - b3, b4

            // c_6 = a_3 * b_4 + a_4 * b_3
            { 3 + 0 + 0 } OP_ROLL
            { 0 + 1 + 0 } OP_PICK
            { Lookup8BitGadget::lookup(k + 3 + 0) }
            { 2 + 0 + 1 } OP_PICK
            { 1 + 1 + 1 } OP_ROLL
            { Lookup8BitGadget::lookup(k + 2 + 1) }
            OP_ADD OP_TOALTSTACK

            // c_7 = a_4 * b_4
            { Lookup8BitGadget::lookup(k + 0) }

            // double c_7
            OP_DUP OP_ADD

            OP_FROMALTSTACK

            // double c_6
            OP_DUP OP_ADD

            OP_FROMALTSTACK

            // double c_5
            OP_DUP OP_ADD

            // pull c_4
            OP_FROMALTSTACK

            // pull c_3
            OP_FROMALTSTACK
            4 OP_ROLL OP_ADD

            // pull c_2
            OP_FROMALTSTACK
            4 OP_ROLL OP_ADD

            // pull c_1
            OP_FROMALTSTACK
            4 OP_ROLL OP_ADD
        }
    }

    pub fn reduce() -> Script {
        // Input:
        //   c4, c3, c2, c1

        script! {
            3 OP_ROLL

            // pull q and save a copy in the altstack
            OP_HINT
            OP_DUP OP_TOALTSTACK

            // q <<= 7
            OP_DUP OP_ADD OP_DUP OP_ADD OP_DUP OP_ADD OP_DUP OP_ADD
            OP_DUP OP_ADD OP_DUP OP_ADD OP_DUP OP_ADD

            // t = c4 - (q << 7)
            OP_SUB

            // stack:
            //   c3, c2, c1, c4 - (q << 7)

            OP_SIZE OP_NOT OP_NOTIF
                OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_SWAP OP_CAT
            OP_ENDIF

            3 OP_ROLL OP_ADD

            // stack:
            //   c2, c1, (c4 - (q << 7)) << 8 + c3

            OP_SIZE OP_NOT OP_NOTIF
                OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_SWAP OP_CAT
            OP_ENDIF

            OP_ROT OP_ADD

            // stack:
            //   c2, ((c4 - (q << 7)) << 8 + c3 + c1) << 8 + c2

            OP_SIZE OP_NOT OP_NOTIF
                OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_SWAP OP_CAT
            OP_ENDIF

            OP_ADD
            OP_FROMALTSTACK OP_ADD
        }
    }
}

pub struct M31Limbs;

impl M31Limbs {
    pub fn add_limbs(a: &[i64], b: &[i64]) -> Vec<i64> {
        assert_eq!(a.len(), 4);
        assert_eq!(b.len(), 4);

        let mut res = vec![0; 4];
        res[0] = a[0] + b[0];
        if res[0] >= 256 {
            res[0] -= 256;
            res[1] += 1;
        }
        res[1] += a[1] + b[1];
        if res[1] >= 256 {
            res[1] -= 256;
            res[2] += 1;
        }
        res[2] += a[2] + b[2];
        if res[2] >= 256 {
            res[2] -= 256;
            res[3] += 1;
        }
        res[3] += a[3] + b[3];

        res
    }
}

pub struct M31LimbsGadget;

impl M31LimbsGadget {
    // a1, ..., a4
    // b1, ..., b4
    pub fn add_limbs() -> Script {
        script! {
            7 OP_ROLL
            { 3 + 1 } OP_ROLL
            OP_ADD
            OP_DUP 256 OP_GREATERTHANOREQUAL
            OP_IF
                256 OP_SUB
                { 1 }
            OP_ELSE
                { 0 }
            OP_ENDIF

            // a2, a3, a4
            // b2, b3, b4
            // c1, carry

            { 5 + 2 } OP_ROLL
            { 2 + 1 + 2 } OP_ROLL
            OP_ADD OP_ADD
            OP_DUP 256 OP_GREATERTHANOREQUAL
            OP_IF
                256 OP_SUB
                { 1 }
            OP_ELSE
                { 0 }
            OP_ENDIF

            // a3, a4
            // b3, b4
            // c1, c2, carry

            { 3 + 3 } OP_ROLL
            { 1 + 1 + 3 } OP_ROLL
            OP_ADD OP_ADD
            OP_DUP 256 OP_GREATERTHANOREQUAL
            OP_IF
                256 OP_SUB
                { 1 }
            OP_ELSE
                { 0 }
            OP_ENDIF

            // a4
            // b4
            // c1, c2, c3, carry
            { 1 + 4 } OP_ROLL
            { 0 + 1 + 4 } OP_ROLL
            OP_ADD OP_ADD
            OP_DUP 128 OP_GREATERTHANOREQUAL
            OP_IF
                128 OP_SUB
                OP_2SWAP OP_SWAP OP_1ADD OP_SWAP OP_2SWAP
            OP_ENDIF

            // c1, c2, c3, c4
            // note: c4 could be a little bit larger, but our program can handle it
        }
    }
}

#[cfg(test)]
mod test {
    use crate::m31::{M31Limbs, M31LimbsGadget, M31Mult, M31MultGadget};
    use crate::table::generate_table;
    use crate::utils::m31_to_limbs;
    use bitcoin_circle_stark::tests_utils::report::report_bitcoin_script_size;
    use bitcoin_circle_stark::treepp::*;
    use bitcoin_script::script;
    use bitcoin_scriptexec::execute_script;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;

    #[test]
    fn test_hypothesis() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        for _ in 0..1 {
            let a = prng.gen_range(0u32..((1 << 31) - 1));
            let b = prng.gen_range(0u32..((1 << 31) - 1));

            let c_limbs = M31Mult::compute_c_limbs(&[(a, b)]).unwrap();
            let q = M31Mult::compute_q(&c_limbs).unwrap();

            let expected = (a as i64) * (b as i64) % ((1 << 31) - 1);

            let mut t = c_limbs[3];
            t = t.checked_sub((q as i32).checked_shl(7).unwrap()).unwrap();
            t = t.checked_shl(8).unwrap();
            t = t.checked_add(c_limbs[2]).unwrap();
            t = t.checked_shl(8).unwrap();
            t = t.checked_add(c_limbs[1]).unwrap();
            t = t.checked_shl(8).unwrap();
            t = t.checked_add(q as i32).unwrap();
            t = t.checked_add(c_limbs[0]).unwrap();

            assert_eq!(t as i64, expected);
        }
    }

    #[test]
    fn test_compute_c_limbs() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        report_bitcoin_script_size(
            "m31",
            "compute_c_limbs",
            M31MultGadget::compute_c_limbs(0).len(),
        );

        for i in 0..100 {
            let a = prng.gen_range(0u32..((1 << 31) - 1));
            let b = prng.gen_range(0u32..((1 << 31) - 1));

            let a_limbs = m31_to_limbs(a);
            let b_limbs = m31_to_limbs(b);

            let c_limbs = M31Mult::compute_c_limbs(&[(a, b)]).unwrap();

            let table = generate_table::<9>();

            let script = script! {
                { &table }
                for _ in 0..i {
                    { 1 }
                }
                { a_limbs.to_vec() }
                { b_limbs.to_vec() }
                { M31MultGadget::compute_c_limbs(i) }
                for c_limb in c_limbs.iter() {
                    { *c_limb }
                    OP_EQUALVERIFY
                }
                for _ in 0..i {
                    OP_DROP
                }
                for _ in 0..256 {
                    OP_2DROP
                }
                OP_TRUE
            };

            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_reduce() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        report_bitcoin_script_size("m31", "reduce", M31MultGadget::reduce().len());

        for _ in 0..100 {
            let a = prng.gen_range(0u32..((1 << 31) - 1));
            let b = prng.gen_range(0u32..((1 << 31) - 1));

            let c_limbs = M31Mult::compute_c_limbs(&[(a, b)]).unwrap();
            let q = M31Mult::compute_q(&c_limbs).unwrap();
            let r = (a as i64) * (b as i64) % ((1 << 31) - 1);

            let script = script! {
                { q }
                for c_limb in c_limbs.iter().rev() {
                    { *c_limb }
                }
                { M31MultGadget::reduce() }
                { r }
                OP_EQUAL
            };

            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_add_limbs() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        report_bitcoin_script_size("m31_limbs", "add_limbs", M31LimbsGadget::add_limbs().len());

        let a = prng.gen_range(0u32..((1 << 31) - 1));
        let b = prng.gen_range(0u32..((1 << 31) - 1));

        let a_limbs = m31_to_limbs(a);
        let b_limbs = m31_to_limbs(b);

        let d = prng.gen_range(0u32..((1 << 31) - 1));
        let e = prng.gen_range(0u32..((1 << 31) - 1));

        let d_limbs = m31_to_limbs(d);
        let e_limbs = m31_to_limbs(e);

        let a_plus_d_limbs = M31Limbs::add_limbs(&a_limbs, &d_limbs);
        let b_plus_e_limbs = M31Limbs::add_limbs(&b_limbs, &e_limbs);

        let table = generate_table::<9>();

        let mut c_limbs = vec![0i64; 4];

        for i in 0..4 {
            for j in 0..4 {
                if i + j >= 4 {
                    c_limbs[i + j - 4] += (a_plus_d_limbs[i] * b_plus_e_limbs[j]) * 2;
                } else {
                    c_limbs[i + j] += a_plus_d_limbs[i] * b_plus_e_limbs[j];
                }
            }
        }

        let script = script! {
            { &table }
            { a_plus_d_limbs.to_vec() }
            { b_plus_e_limbs.to_vec() }
            { M31MultGadget::compute_c_limbs(0) }
            for c_limb in c_limbs.iter() {
                { *c_limb }
                OP_EQUALVERIFY
            }
            for _ in 0..256 {
                OP_2DROP
            }
            OP_TRUE
        };

        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }
}
