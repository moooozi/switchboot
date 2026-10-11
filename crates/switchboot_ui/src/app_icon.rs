//! The window and platform packaging share assets generated from the root app-icon.svg.
use std::io::Cursor;

pub fn window_icon() -> iced::window::Icon {
    let bytes = include_bytes!("../../switchboot_core/icons/128x128.png");
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().expect("decode embedded app icon");
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut pixels)
        .expect("read embedded app icon pixels");
    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "app icon must be RGBA; regenerate with packaging/generate-app-icons.mjs"
    );
    pixels.truncate(info.buffer_size());
    iced::window::icon::from_rgba(pixels, info.width, info.height)
        .expect("embedded app icon dimensions match its pixels")
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_icon_decodes_with_transparency_and_visible_artwork() {
        let (pixels, size) = super::window_icon().into_raw();
        assert_eq!((size.width, size.height), (128, 128));
        assert_eq!(pixels.len(), 128 * 128 * 4);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 0));
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] == 255));
    }
}
