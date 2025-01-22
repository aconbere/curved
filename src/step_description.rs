#[derive(Debug)]
pub struct StepDescription {
    pub count: u32,
    pub columns: u32,
    pub width: u32,
    pub height: u32,
    pub max_tone: u32,
    pub interval: u32,
    pub square_size: u32,
    pub rows: u32,
}

impl StepDescription {
    pub fn new(count: u32, columns: u32, width: u32, max_tone: u32) -> Self {
        let interval = (max_tone as f32 / count as f32).ceil() as u32;
        let square_size = (width as f32 / columns as f32).ceil() as u32;
        let rows = (count as f32 / columns as f32).ceil() as u32;
        let height = rows * square_size;

        StepDescription {
            count,
            columns,
            width,
            height,
            max_tone,
            interval,
            square_size,
            rows,
        }
    }
}
