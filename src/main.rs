use device_query::{DeviceQuery, DeviceState, Keycode};

fn main() {
    let device_state = DeviceState::new();
    loop {
        let keys: Vec<Keycode> = device_state.get_keys();
        if !keys.is_empty() {
            println!("{:?}", keys);
        }
    }
}

