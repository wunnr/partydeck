use std::error::Error;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::path::PathBuf;

use wayland_client::Connection;
use wayland_client::Dispatch;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_registry::{Event as RegistryEvent};
use wayland_client::protocol::wl_seat::WlSeat;

use crate::layout_manager::river_layout_v3::river_layout_v3;
use crate::layout_manager::wayland_client_code::river_layout_v3::river_layout_v3::RiverLayoutV3;
use crate::layout_manager::wayland_client_code::river_layout_v3::river_layout_manager_v3::RiverLayoutManagerV3;
use crate::layout_manager::wayland_client_code::river_control_unstable_v1::zriver_command_callback_v1::ZriverCommandCallbackV1;
use crate::layout_manager::wayland_client_code::river_control_unstable_v1::zriver_control_v1::ZriverControlV1;
use std::env;


struct LayoutState {
    river_layout: Option<RiverLayoutV3>,
    river_layout_man: Option<RiverLayoutManagerV3>,
    river_control: Option<ZriverControlV1>,
    seat: Option<WlSeat>,
    outputs_name: u32,
    outputs: usize,
}
impl Dispatch<WlRegistry, ()> for LayoutState {
    fn event(
            state: &mut Self,
            registry: &WlRegistry,
            event: <WlRegistry as wayland_client::Proxy>::Event,
            _: &(),
            _conn: &Connection,
            qh: &wayland_client::QueueHandle<Self>,
        ) {
        match event {
            RegistryEvent::Global { name, interface, version } => {
                // println!("OFFERED RESOURCE: {} {} {}", name, interface, version);

                if interface == "zriver_control_v1" {
                    let control = registry.bind::<ZriverControlV1, _, _>(name, version, qh, ());
                    state.river_control = Some(control);
                }
                if interface == "river_layout_manager_v3" {
                    let manager = registry.bind::<RiverLayoutManagerV3, _, _>(name, version, qh, ());
                    state.river_layout_man = Some(manager);
                }
                if interface == "wl_seat" {
                    let seat = registry.bind::<WlSeat, _, _>(name, version, qh, ());
                    state.seat = Some(seat);
                }
                if interface == "wl_output" {
                    let output = registry.bind::<WlOutput, _, _>(name, version, qh, ());
                    if let Some(manager) = &state.river_layout_man {
                        state.river_layout = Some(manager.get_layout(&output, "river-game-layout".to_string(), qh, ()));
                        

                        if let Some(ctrl) = &state.river_control {
                            if let Some(seat) = &state.seat {
                                println!("Enabling layout!");

                                ctrl.add_argument("output-layout".to_string());
                                ctrl.add_argument("river-game-layout".to_string());
                                ctrl.run_command(&seat, qh, ());
                            }
                        }
                    }
                    state.outputs_name = name;
                    state.outputs+=1;
                    // println!("Added one to wl output! NEW: {}", state.outputs);
                }
            }
            RegistryEvent::GlobalRemove { name } => {
                // println!("REMOVED RESOURCE: {}, removal: {}", name, state.outputs_name);
                if name == state.outputs_name {
                    state.outputs = state.outputs.saturating_sub(1); // Prevent errors from underflow
                    // println!("New outputs count: {}", state.outputs);
                    if state.outputs == 0 {
                        if let Some(ctrl) = &state.river_control {
                            if let Some(seat) = &state.seat {
                                println!("Exiting!!!");

                                ctrl.add_argument("exit".to_string());
                                ctrl.run_command(seat, qh, ());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<RiverLayoutV3, ()> for LayoutState {
    fn event(
        state: &mut Self,
        _proxy: &RiverLayoutV3,
        event: <RiverLayoutV3 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            river_layout_v3::Event::LayoutDemand { view_count, usable_width, usable_height, tags: _, serial } => {
                if let Some(layout) = &state.river_layout {
                    let (mut w,mut h) = (0, 0);

                    while w * h < view_count {
                        if usable_width * h * 9 > usable_height * w * 16 {
                            w+=1;
                        } else {
                            h+=1;
                        }
                    }

                    while view_count <= w*(h-1) {h-=1};
                    while view_count <= (w-1)*h {w-=1};


                    for i in 0..view_count {
                        let mut cur_row_width = w - ((i%h >= (view_count-1)%h+1) as u32);

                        if cur_row_width == 0 {
                            cur_row_width = 1;
                        }

                        layout.push_view_dimensions(
                            ((i / h) * usable_width / cur_row_width) as i32,
                            ((i % h) * usable_height / h) as i32,
                            (usable_width / cur_row_width) as u32,
                            (usable_height / h) as u32,
                            serial
                        );
                    }
                    layout.commit("[]=".to_string(), serial);
                }
            }
            _ => {}
        }
    }
}

macro_rules! impl_empty_dispatch {
    ($state:ty, $iface:ty) => {
        impl Dispatch<$iface, ()> for $state {
            fn event(
                    _state: &mut $state,
                    _proxy: &$iface,
                    _event: <$iface as wayland_client::Proxy>::Event,
                    _data: &(),
                    _conn: &Connection,
                    _qhandle: &wayland_client::QueueHandle<$state>,
                ) {
                // ignore
            }
        }
    };
}

impl_empty_dispatch!(LayoutState,ZriverCommandCallbackV1);
impl_empty_dispatch!(LayoutState,ZriverControlV1);
impl_empty_dispatch!(LayoutState,RiverLayoutManagerV3);
impl_empty_dispatch!(LayoutState,WlSeat);
impl_empty_dispatch!(LayoutState,WlOutput);



pub fn start_layout_manager(_fd: i32) {
    let mut file = unsafe { std::fs::File::from_raw_fd(_fd) };

    file.write_all(
    env::var_os("WAYLAND_DISPLAY")
            .expect("Failed to decode wayland display")
            .as_encoded_bytes()
    ).expect("Unable to write to fd!");

    let _ = file.flush();
    drop(file);

    let conn = Connection::connect_to_env().expect("Failed to connect to wayland session");
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let _registry = conn.display().get_registry(&qh, ());
    let mut state = LayoutState {
        river_layout: None,
        river_layout_man: None,
        river_control: None,
        seat: None,
        outputs_name: 0,
        outputs: 0
    };

    loop {
        if let Err(e) = event_queue.blocking_dispatch(&mut state) {
            eprintln!("Wayland connection closed {e}");
            break;
        }
    }
}


// Sends the splitscreen script to the active KWin session through DBus
pub fn kwin_dbus_start_script(file: PathBuf) -> Result<(), Box<dyn Error>> {
    println!(
        "[partydeck] util::kwin_dbus_start_script - Loading script {}...",
        file.display()
    );
    if !file.exists() {
        return Err("[partydeck] util::kwin_dbus_start_script - Script file doesn't exist!".into());
    }

    let conn = zbus::blocking::Connection::session()?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;

    let _: i32 = proxy.call("loadScript", &(file.to_string_lossy(), "splitscreen"))?;
    println!("[partydeck] util::kwin_dbus_start_script - Script loaded. Starting...");
    let _: () = proxy.call("start", &())?;

    println!("[partydeck] util::kwin_dbus_start_script - KWin script started.");
    Ok(())
}

pub fn kwin_dbus_unload_script() -> Result<(), Box<dyn Error>> {
    println!("[partydeck] util::kwin_dbus_unload_script - Unloading splitscreen script...");
    let conn = zbus::blocking::Connection::session()?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )?;

    let _: bool = proxy.call("unloadScript", &("splitscreen"))?;

    println!("[partydeck] util::kwin_dbus_unload_script - Script unloaded.");
    Ok(())
}