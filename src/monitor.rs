use eframe::UserEvent;
use winit::event_loop::EventLoop;
use winit::platform::run_on_demand::EventLoopExtRunOnDemand;

#[derive(Clone)]
pub struct Monitor {
    name: String,
    width: u32,
    height: u32,
}

impl Monitor {
    pub fn new(name: String, width: u32, height: u32) -> Self {
        Monitor {
            name,
            width,
            height,
        }
    }

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

// Monitor detection. Each method has its pros and cons

pub fn get_monitors_sdl() -> Vec<Monitor> {
    let video = sdl2::init().unwrap().video().unwrap();
    println!("driver: {}", video.current_video_driver());

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

pub fn get_monitors_winit(eventloop: &mut EventLoop<UserEvent>) -> Vec<Monitor> {
    let mut mons = Vec::<Monitor>::new();

    eventloop
        .run_on_demand(|_, event_loop| {
            if !mons.is_empty() {
                return;
            }
            let mut monitors = Vec::new();
            for monitor in event_loop.available_monitors() {
                monitors.push(Monitor::new(
                    monitor.name().unwrap(),
                    monitor.size().width,
                    monitor.size().height,
                ));
            }

            mons = monitors;
            event_loop.exit();
        })
        .unwrap();
    mons
}
