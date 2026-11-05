//! Accessor helpers used by the generated record types.
use crate::numeric::{decode, encode, Dec, Pic};

pub fn get_n(b: &[u8], pic: Pic) -> Dec {
    decode(b, pic)
}
pub fn set_n(b: &mut [u8], pic: Pic, v: Dec) {
    encode(v, pic, b)
}
pub fn set_edited(b: &mut [u8], pic: &str, v: Dec) {
    crate::text::mv(b, crate::edit::edit(v, pic).as_bytes())
}
