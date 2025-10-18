use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;

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


fn get_monitors_x11() -> Result<Vec<Monitor>, Box<dyn std::error::Error>> {
    let (con, screen_num) = x11rb::connect(None)?;
    let screen = &con.setup().roots[screen_num];

    let res = con.randr_get_screen_resources(screen.root)?.reply()?;
    let mut monitors = Vec::new();
    for output in res.outputs {
        if let Ok(info) = con.randr_get_output_info(output, 0)?.reply() {
            if info.crtc != x11rb::NONE {
                if let Ok(crtc) = con.randr_get_crtc_info(info.crtc, 0)?.reply() {
                    monitors.push(Monitor {
                        name: String::from_utf8_lossy(&info.name).into(),
                        width: crtc.width.into(),
                        height: crtc.height.into(),
                    });
                }
            }
        }
    }

    Ok(monitors)
}

pub fn get_monitors_direct() -> Vec<Monitor> {
    if let Ok(monitors) = get_monitors_x11() {
        return monitors;
    }

    Vec::new()
}