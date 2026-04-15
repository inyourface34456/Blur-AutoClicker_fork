// For some reason, it is esxeedingly complex to just get the DPI on wayland
// I dont know why this is, but i do expect this given how hard the maintainers 
// and developers of wayland cant even agree on how to implament an api to spawn
// a window in a user-specifed location. why do they have to make this so hard?

use wayland_client::{
    protocol::{wl_output, wl_registry},
    Connection, Dispatch, QueueHandle,
};

#[derive(Default)]
struct State {
    outputs: Vec<(i32, i32, i32, i32)>, // (mm_w, mm_h, px_w, px_h)
    pending: (i32, i32, i32, i32),
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(state: &mut Self, registry: &wl_registry::WlRegistry, event: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == "wl_output" {
                registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(state: &mut Self, _: &wl_output::WlOutput, event: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_output::Event::Geometry { physical_width, physical_height, .. } => {
                state.pending.0 = physical_width;
                state.pending.1 = physical_height;
            }
            wl_output::Event::Mode { width, height, flags, .. }
                if matches!(flags, wayland_client::WEnum::Value(v) if v.contains(wl_output::Mode::Current)) =>
            {
                state.pending.2 = width;
                state.pending.3 = height;
            }
            // Done fires after all properties for an output have been sent
            wl_output::Event::Done => {
                state.outputs.push(std::mem::take(&mut state.pending));
            }
            _ => {}
        }
    }
}

pub fn get_wayland_dpi() -> Result<f64, String> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue = conn.new_event_queue::<State>();
    conn.display().get_registry(&queue.handle(), ());

    let mut state = State::default();
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?; // bind wl_output globals
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?; // receive Geometry + Mode + Done events

    let dpis: Vec<f64> = state.outputs.iter()
        .filter(|(mw, mh, pw, ph)| *mw > 0 && *mh > 0 && *pw > 0 && *ph > 0)
        .map(|(mw, mh, pw, ph)| {
            ((*pw as f64 / *mw as f64) + (*ph as f64 / *mh as f64)) / 2.0 * 25.4
        })
        .collect();

    Ok(if dpis.is_empty() { 96.0 } else { dpis.iter().sum::<f64>() / dpis.len() as f64 })
}