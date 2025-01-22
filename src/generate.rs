use ab_glyph::FontRef;
use anyhow;
use image::{DynamicImage, ImageBuffer, Luma};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;

use super::step_description::StepDescription;

type Gray16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

const BLACK: u32 = 0;

const LATO_BLACK_BYTES: &[u8] = include_bytes!("../data/fonts/Lato-Black.ttf");

/* Creates a new step wedge image
 * 0 is black
 * 65536 is white
 *
 * divide the range by count then draw that value into each square
 */
pub fn generate(process: Option<String>, notes: Option<String>) -> anyhow::Result<DynamicImage> {
    let font_lato_black = FontRef::try_from_slice(LATO_BLACK_BYTES)?;

    //  pixels on the margin of the image
    let start_x = 10;
    let start_y = 10;

    let step_description = StepDescription::new(101, 10, 1000, u16::MAX as u32);

    let mut image: Gray16Image =
        ImageBuffer::new(step_description.width + 20, step_description.height + 20);
    draw_steps(
        &mut image,
        &font_lato_black,
        &step_description,
        start_x,
        start_y,
    );

    draw_grid(&mut image, &step_description, start_x, start_y);

    let process_and_notes_x = start_x + step_description.square_size;
    let process_and_notes_y =
        start_y + (step_description.square_size * (step_description.rows - 1));
    draw_process_and_notes(
        &mut image,
        &font_lato_black,
        &step_description,
        process_and_notes_x,
        process_and_notes_y,
        process,
        notes,
    );

    Ok(DynamicImage::ImageLuma16(image))
}

fn draw_steps(
    image: &mut Gray16Image,
    font: &FontRef,
    step_description: &StepDescription,
    start_x: u32,
    start_y: u32,
) {
    let mut n = 0;
    for row in 0..step_description.rows {
        for col in 0..step_description.columns {
            // stop when we reach count
            if n >= step_description.count {
                break;
            }
            let x = start_x + (col * step_description.square_size);
            let y = start_y + (row * step_description.square_size);
            let tone = step_description.interval * n;

            let rect = Rect::at(x as i32, y as i32)
                .of_size(step_description.square_size, step_description.square_size);
            draw_filled_rect_mut(image, rect, Luma([tone as u16]));

            // flip the foreground color half way through to preserve contrast
            let foreground_color = if n < step_description.count / 2 {
                step_description.max_tone
            } else {
                BLACK
            };

            // draw a count on the square. this i useful for hand analysis
            draw_text_mut(
                image,
                Luma([foreground_color as u16]),
                x as i32 + 5,
                y as i32 + 5,
                20 as f32,
                font,
                &format!("{}", n),
            );

            n += 1;
        }
    }
}

fn draw_grid(
    image: &mut Gray16Image,
    step_description: &StepDescription,
    start_x: u32,
    start_y: u32,
) {
    // Draw the horizontal grid lines
    for row in 0..step_description.rows {
        // Flip the foreground color from white to black half way through to preserve contrast
        let foreground_color = if row < step_description.rows / 2 {
            step_description.max_tone
        } else {
            BLACK
        };
        let y = ((row * step_description.square_size) + start_y) as i32;
        let squares_width = step_description.square_size * step_description.columns;
        let rect = Rect::at(start_x as i32, y).of_size(squares_width, 2);

        draw_filled_rect_mut(image, rect, Luma([foreground_color as u16]));
    }

    // Draw the vertical grid lines
    for col in 0..(step_description.columns + 1) {
        // pick a generic middle grey
        let tone = step_description.max_tone / 2;
        let x = ((col * step_description.square_size) + start_x) as i32;

        // stop early after the first row so we can have some
        // room to draw text for notes and process
        let height = if col > 0 {
            step_description.square_size * (step_description.rows - 1)
        } else {
            step_description.square_size * step_description.rows
        };

        let rect = Rect::at(x, start_y as i32).of_size(2, height);
        draw_filled_rect_mut(image, rect, Luma([tone as u16]));
    }
}

fn draw_process_and_notes(
    image: &mut Gray16Image,
    font: &FontRef,
    step_description: &StepDescription,
    start_x: u32,
    start_y: u32,
    process: Option<String>,
    notes: Option<String>,
) {
    let margin = 25;
    if let Some(notes) = notes {
        draw_text_mut(
            image,
            Luma([step_description.max_tone as u16]),
            start_x as i32 + margin,
            start_y as i32 + margin,
            20 as f32,
            font,
            format!("Process: {}", notes).as_str(),
        );
    }
    if let Some(process) = process {
        // Draw notes and process
        draw_text_mut(
            image,
            Luma([step_description.max_tone as u16]),
            start_x as i32 + margin,
            start_y as i32 + margin + 20,
            20 as f32,
            font,
            format!("Notes: {}", process).as_str(),
        );
    }
}
