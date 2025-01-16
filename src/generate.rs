use ab_glyph::FontRef;
use image::{DynamicImage, Luma};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

/* Creates a new step wedge image
 * 0 is black
 * 65536 is white
 *
 * divide the range by count then draw that value into each square
 */
pub fn generate(debug: bool) -> DynamicImage {
    const FONT_BYTES: &[u8] = include_bytes!("../data/fonts/Lato-Black.ttf");
    let font = FontRef::try_from_slice(FONT_BYTES).unwrap();

    let count = 101;
    let columns = 10;
    let rows = (count as f32 / columns as f32).ceil() as u32;

    // number of pixels each square
    let square_size: u32 = 100;

    // count of pixels on the margin of the image
    let margin = 10;

    let width = columns * square_size + (margin * 2);
    let height = (rows * square_size) + (margin * 2);

    let mut image = DynamicImage::new_luma16(width, height).to_luma16();

    // the amount that each square increases as we go towards max
    // Note that because we start at zero we want 100 equal chunks
    // to filled up 101 times
    let interval = u16::MAX / (count - 1);

    let mut n = 0;
    for row in 0..rows {
        for col in 0..columns {
            let x = (margin + (col * square_size)) as i32;
            let y = (margin + (row * square_size)) as i32;

            let tone = interval * n;
            if debug {
                println!("tone: {}", tone);
            }
            let rect = Rect::at(x, y).of_size(square_size, square_size);
            draw_filled_rect_mut(&mut image, rect, Luma([tone]));

            // flip the foreground color half way through to preserve contrast
            let foreground_color = if n < count / 2 { u16::MAX } else { 0 };

            // draw a count on the square. this i useful for hand analysis
            draw_text_mut(
                &mut image,
                Luma([foreground_color]),
                x + 5,
                y + 5,
                20 as f32,
                &font,
                format!("{}", n).as_str(),
            );

            // stop when we reach count
            n += 1;
            if n == count {
                break;
            }
        }
    }

    // Draw the horizontal grid lines
    for row in 0..rows {
        // Flip the foreground color from white to black half way through to preserve contrast
        let foreground_color = if row < rows / 2 { u16::MAX } else { 0 };
        let y = ((row * square_size) + margin) as i32;
        let squares_width = square_size * columns;
        let rect = Rect::at(margin as i32, y).of_size(squares_width, 2);
        draw_filled_rect_mut(&mut image, rect, Luma([foreground_color]));
    }

    // Draw the vertical grid lines
    for col in 0..(columns + 1) {
        // pick a generic middle grey
        let tone = u16::MAX / 2;
        let x = ((col * square_size) + margin) as i32;
        let squares_height = square_size * rows;
        let rect = Rect::at(x, margin as i32).of_size(2, squares_height);
        draw_filled_rect_mut(&mut image, rect, Luma([tone]));
    }

    DynamicImage::ImageLuma16(image)
}
