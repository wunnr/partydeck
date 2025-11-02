#[derive(Clone)]
pub struct Monitor {
    name: String,
    width: u32,
    height: u32,
}

impl Monitor {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

// Using sdl seems to be the most reliable way to get monitor info in a way that lines up with what gamescope expects, since its --display-index option uses sdl.
pub fn get_monitors_sdl() -> Vec<Monitor> {
    let video = sdl2::init().unwrap().video().unwrap();
    let count = video.num_video_displays().unwrap();
    let mut monitors = Vec::new();
    for i in 0..count {
        monitors.push(Monitor {
            name: video.display_name(i).unwrap(),
            width: video.display_bounds(i).unwrap().width(),
            height: video.display_bounds(i).unwrap().height(),
        });
    }
    monitors
}
