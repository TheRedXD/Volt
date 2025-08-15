#![warn(clippy::nursery, clippy::pedantic, clippy::undocumented_unsafe_blocks, clippy::allow_attributes_without_reason)]
pub mod device;
pub mod processing;

pub mod utils {
    use std::mem::{transmute_copy, ManuallyDrop, MaybeUninit};

    // https://internals.rust-lang.org/t/should-there-by-an-array-zip-method/21611/5
    pub fn zip<T, U, const N: usize>(ts: [T; N], us: [U; N]) -> [(T, U); N] {
        let mut ts = ts.map(ManuallyDrop::new);
        let mut us = us.map(ManuallyDrop::new);
        let mut zip = [const { MaybeUninit::<(T, U)>::uninit() }; N];
        for i in 0..N {
            // SAFETY: ts[i] taken once, untouched afterwards
            let t = unsafe { ManuallyDrop::take(&mut ts[i]) };
            // SAFETY: us[i] taken once, untouched afterwards
            let u = unsafe { ManuallyDrop::take(&mut us[i]) };
            zip[i].write((t, u));
        }
        // SAFETY: zip has been fully initialized
        unsafe { transmute_copy(&zip) }
    }
}
