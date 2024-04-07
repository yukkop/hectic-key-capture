use device_query::{DeviceQuery, DeviceState, Keycode};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;

fn main() {
    let device_state = DeviceState::new();
    let mut key_counts: HashMap<Keycode, u32> = HashMap::new();
    let mut last_keys = Vec::new();

    loop {
        let keys = device_state.get_keys();

        // Check for new key presses
        for key in &keys {
            if !last_keys.contains(key) {
                *key_counts.entry(*key).or_insert(0) += 1;
                println!("{:?} has been pressed {} times", key, key_counts[key]);
            }
        }

        last_keys = keys;

        thread::sleep(Duration::from_millis(100)); // Reduce CPU usage
    }
}

