use colored::*;
use crossterm::event::{poll, KeyModifiers};
use crossterm::{
    event::{read, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::collections::HashMap;
use std::env;
use std::io::stdout;
use std::time::Duration;

const PRODUCTIVE_SENSITIVITY_KEY: &str = "productive";
const PRODUCTIVE_SENSITIVITY_VALUE: u64 = 100;
const INTENT_SENSITIVITY_KEY: &str = "intent";
const INTENT_SENSITIVITY_VALUE: u64 = 1;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const SENSITIVITY_SHORT: &str = "-s";
const SENSITIVITY_LONG: &str = "--sensitivity";

const HELP_SHORT: &str = "-h";
const HELP_LONG: &str = "--help";

const VERSION_SHORT: &str = "-V";
const VERSION_LONG: &str = "--version";

const VERBOSE_SHORT: &str = "-v";
const VERBOSE_LONG: &str = "--verbose";

macro_rules! verbose {
    ($verbose:expr, $($arg:tt)*) => {
        if $verbose {
            println!("\r{}", format!($($arg)*));
        }
    };
}
fn main() {
    let mut sensitivity = PRODUCTIVE_SENSITIVITY_VALUE;
    let mut verbose = false;
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
            VERSION_SHORT | VERSION_LONG => {
                println!("{}", VERSION);
            }
            VERBOSE_SHORT | VERBOSE_LONG => verbose = true,
            HELP_SHORT | "-?" | "?" | "h" | HELP_LONG | "-help" | "help" => {
                println!(
                    r#"Program for capture statistic of you keyboard usage

{usage_title} {usage_content}

{optiongs_title}
    {sensitivity_short}, {sensitivity_long} {sensitivity_value}
                    Interprets how often keyboard input will be taken (milliseconds)
                    for reduce CPU usage

                    100 is a standard value, for a typical keyboard it will be enough
                    1 - very sensitive
                    
                    If your keyboard can handle different modes of keystrokes, 
                    it is possible that some inputs will be too short to not be picked up.
                    
                    It may be worth checking whether all the keys are picked up by the 
                    program and if some cannot be picked up, reduce this value

    {version_short}, {version_long}         
                    Show the version

    {verbose_short}, {verbose_long}         
                    Describe the steps of the program

    {help_short}, {help_long}         
                    This message"#,
                    usage_title = "Usage:".green(),
                    optiongs_title = "Options:".green(),
                    usage_content = format!("{program_name} [OPTIONS]").cyan(),
                    sensitivity_short = SENSITIVITY_SHORT.cyan(),
                    sensitivity_long = SENSITIVITY_LONG.cyan(),
                    sensitivity_value = format!(
                        "[<pos_val> | {PRODUCTIVE_SENSITIVITY_KEY} | {INTENT_SENSITIVITY_KEY}]"
                    )
                    .cyan(),
                    help_long = HELP_LONG.cyan(),
                    help_short = HELP_SHORT.cyan(),
                    version_short = VERSION_SHORT.cyan(),
                    version_long = VERSION_LONG.cyan(),
                    verbose_short = VERBOSE_SHORT.cyan(),
                    verbose_long = VERBOSE_LONG.cyan(),
                );

                std::process::exit(0);
            }
            _ => println!("Unhandled option: {}", arg),
        }
    }

    let device_state = DeviceState::new();
    let mut key_counts: HashMap<Keycode, u32> = HashMap::new();
    let mut last_keys = Vec::new();

        let mut stdout = stdout();

    if verbose {
        enable_raw_mode().expect("enable_raw_mode problem");
        execute!(stdout, EnterAlternateScreen).expect("EnterAlternateScreen problem");
    }

    loop {
        let keys = device_state.get_keys();

        // Check for new key presses
        for key in &keys {
            if !last_keys.contains(key) {
                *key_counts.entry(*key).or_insert(0) += 1;
                verbose!(
                    verbose,
                    "{:?} has been pressed {} times",
                    key,
                    key_counts[key]
                );
            }
        }

        last_keys = keys;

        if verbose && poll(Duration::from_millis(sensitivity)).expect("poll error") {
            match read().expect("read error") {
                Event::Key(event) => {
                    if event.code == KeyCode::Char('c')
                        && event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    // Handle other key events here
                }
                _ => {}
            }
        }
    }

    if verbose {
        execute!(stdout, LeaveAlternateScreen).expect("LeaveAlternateScreen problem");
        disable_raw_mode().expect("disable_raw_mode problem");
    }
}
