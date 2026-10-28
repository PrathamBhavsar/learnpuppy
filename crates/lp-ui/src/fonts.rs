/// Geist Mono variable font, Latin subset (weights 100–900).
/// Copied from Next.js's `dist/next-devtools/server/font/geist-mono-latin.woff2`.
/// SIL Open Font License 1.1; see `assets/geist-mono-LICENSE.txt`.
/// Use as a `data:font/woff2;base64` URL in an `@font-face` with
/// `font-family: 'Geist Mono'` and `font-weight: 100 900`.
pub const GEIST_MONO_WOFF2: &[u8] = include_bytes!("../assets/geist-mono-latin.woff2");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geist_mono_embeds_a_complete_woff2() {
        assert_eq!(&GEIST_MONO_WOFF2[..4], b"wOF2");
        let length = u32::from_be_bytes(GEIST_MONO_WOFF2[8..12].try_into().unwrap());
        assert_eq!(length as usize, GEIST_MONO_WOFF2.len());
    }
}
