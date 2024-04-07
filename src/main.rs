use device_query::{DeviceQuery, DeviceState, Keycode};
use std::collections::HashMap;
use std::time::Duration;
use std::{env, thread};

const PRODUCTIVE_SENSITIVITY_KEY: &str = "productive";
const PRODUCTIVE_SENSITIVITY_VALUE: u64 = 100;
const INTENT_SENSITIVITY_KEY: &str = "intent";
const INTENT_SENSITIVITY_VALUE: u64 = 1;

const SENSITIVITY_SHORT: &str = "-s";
const SENSITIVITY_LONG: &str = "--sensitivity";

const HELP_SHORT: &str = "-h";
const HELP_LONG: &str = "--help";

fn main() {
    let mut sensitivity: u64 = PRODUCTIVE_SENSITIVITY_VALUE;
    let mut args = env::args();
    let program_name = args.next().expect("this panic not posible");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            SENSITIVITY_SHORT | SENSITIVITY_LONG => {
                sensitivity = match args.next().expect("provide value to sensitivity").as_str() {
                    PRODUCTIVE_SENSITIVITY_KEY => PRODUCTIVE_SENSITIVITY_VALUE,
                    INTENT_SENSITIVITY_KEY => INTENT_SENSITIVITY_VALUE,
                    other => other.parse::<u64>().expect(
                        format!(
                            "value for sensitivity must be a number > 0 or {} or {}",
                            PRODUCTIVE_SENSITIVITY_KEY, INTENT_SENSITIVITY_KEY
                        )
                        .as_str(),
                    ),
                }
            }
            HELP_SHORT | "-?" | "?" | "h" | HELP_LONG | "-help" | "help" => {
                println!(
                    r#"{program_name} - program for capture statistic 
of you keyboard usage
{SENSITIVITY_SHORT}, {SENSITIVITY_LONG} [<pos_val> | {PRODUCTIVE_SENSITIVITY_KEY} | {INTENT_SENSITIVITY_KEY}]
                    Interprets how often keyboard input will be taken (milliseconds)
                    100 is a standard value, for a typical keyboard it will be enough
                    1 - very sensitive
                    
                    If your keyboard can handle different modes of keystrokes, 
                    it is possible that some inputs will be too short to not be picked up.
                    
                    It may be worth checking whether all the keys are picked up by the 
                    program and if some cannot be picked up, reduce this value

{HELP_SHORT}, {HELP_LONG}         
                    This message"#
                );
            }
            _ => println!("Unhandled option: {}", arg),
        }
    }

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

        thread::sleep(Duration::from_millis(sensitivity)); // Reduce CPU usage
    }
}
