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
    pub expected_interval: u16,
}

impl StepDescription {
    pub fn new(count: u32, columns: u32, width: u32, max_tone: u32) -> Self {
        let interval = (max_tone as f32 / count as f32).ceil() as u32;
        let square_size = (width as f32 / columns as f32).ceil() as u32;
        let rows = (count as f32 / columns as f32).ceil() as u32;
        let height = rows * square_size;
        let expected_interval = (max_tone / (count - 1)) as u16;

        StepDescription {
            count,
            columns,
            width,
            height,
            max_tone,
            interval,
            square_size,
            rows,
            expected_interval,
        }
    }

    pub fn input_values(&self) -> Vec<u16> {
        (0..self.count)
            .map(|x| x as u16 * &self.expected_interval)
            .collect()
    }
}
