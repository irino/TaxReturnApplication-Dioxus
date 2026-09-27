# PDF field maps

JSON field maps mirror `src/pdf/field_map.rs`.

- Coordinates are millimetres from the **bottom-left** of A4.
- Edit these when aligning to updated National Tax Agency layouts.
- Runtime currently embeds the Rust maps; load-from-JSON can be wired later.

## Fonts

Place a Japanese TTF/OTF in `assets/fonts/` (e.g. `ipag.ttf`) or install system IPA Gothic.
