use ahash::AHasher;
use colored::*;
use core::fmt;
use crossterm::event::{poll, KeyModifiers};
use crossterm::{
    event::{read, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use device_query::{DeviceQuery, DeviceState, Keycode};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_derive::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::hash::BuildHasherDefault;
use std::io::{self, stdout, Read, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};
use std::{env, thread};

/// A [`HashMap`][hashbrown::HashMap] implementing aHash, a high
/// speed keyed hashing algorithm intended for use in in-memory hashmaps.
///
/// aHash is designed for performance and is NOT cryptographically secure.
///
/// Within the same execution of the program iteration order of different
/// `HashMap`s only depends on the order of insertions and deletions,
/// but it will not be stable between multiple executions of the program.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, BuildHasherDefault<AHasher>>;

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

const OUTPUT_SHORT: &str = "-o";
const OUTPUT_LONG: &str = "--output";

const NO_OUTPUT_LONG: &str = "--no-output";

const MODIFY_OUTPUT_SHORT: &str = "-y";
const MODIFY_OUTPUT_LONG: &str = "--modify-output";

const MODIFY_TRACE_SHORT: &str = "-Y";
const MODIFY_TRACE_LONG: &str = "--modify-trace";

const TRACE_SHORT: &str = "-t";
const TRACE_LONG: &str = "--trace";

const PAIRS_SHORT: &str = "-p";
const PAIRS_LONG: &str = "--pairs";

const PLAIN_SHORT: &str = "-P";
const PLAIN_LONG: &str = "--plain-style";

const NO_CHORDS_LONG: &str = "--no-chords";

const DEFAULT_STATISTIC_PATH_YAML: &str = "key-capture-statistic.yaml";

macro_rules! verbose {
    ($verbose:expr, $($arg:tt)*) => {
        if $verbose {
            println!("\r{}", format!($($arg)*));
        }
    };
}

trait Frmater {
    fn to_string(&self) -> &str;
}

impl Frmater for PathBuf {
    fn to_string(&self) -> &str {
        if let Some(str) = self.to_str() {
            str
        } else {
            "%path error%"
        }
    }
}

#[derive(Debug)]
pub enum TraceStep {
    First(Vec<Keycode>),
    Regular(Vec<Keycode>, Duration),
    Empty,
}

#[derive(Debug)]
pub struct KeyCounts {
    pub config: Option<Config>,
    pub map: HashMap<CountItem, u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    pub pairs: bool,
    pub no_chords: bool,
    pub version: String,
}

impl Config {
    pub fn new(pairs: bool, no_chords: bool, version: String) -> Self {
        Self {
            pairs,
            no_chords,
            version,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum CountItem {
    Single(Vec<Keycode>),
    Pair(Vec<Keycode>, Vec<Keycode>),
}

fn keycode_to_string(keycode: &Keycode) -> String {
    format!("{:?}", keycode)
}

fn input_to_string(input: Vec<Keycode>) -> String {
    let keys_str: Vec<String> = input.iter().map(keycode_to_string).collect();
    format!("{:?}", keys_str.join("+")).replace("\"", "")
}

impl Serialize for KeyCounts {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.map.len()))?;

        map.serialize_entry("config", &self.config)?;

        for (key, value) in &self.map {
            let key_str = match key {
                CountItem::Single(input) => {
                    let input_str: String = input_to_string(input.clone());
                    format!("{}", input_str)
                }
                CountItem::Pair(input1, input2) => {
                    let input1_str: String = input_to_string(input1.clone());
                    let input2_str: String = input_to_string(input2.clone());
                    format!("{}, {}", input1_str, input2_str)
                }
            };
            map.serialize_entry(&key_str, value)?;
        }

        map.end()
    }
}

fn parse_keycode_from_string(s: &str) -> Result<Keycode, String> {
    log::trace!("try to parse keycode from {}", s);
    Keycode::from_str(s)
}

fn parse_input_from_string(s: &str) -> Result<Vec<Keycode>, String> {
    let keycodes = s
        .split('+')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(parse_keycode_from_string(trimmed))
            }
        })
        .collect::<Result<Vec<Keycode>, String>>()?;

    Ok(keycodes)
}

fn parse_count_item(s: &str) -> Result<CountItem, String> {
    let inputs = s
        .split(", ")
        .map(parse_input_from_string)
        .collect::<Result<Vec<Vec<Keycode>>, String>>()?;

    if inputs.len() == 1 {
        Ok(CountItem::Single(inputs[0].clone()))
    } else if inputs.len() > 2 {
        Err("Unrecognized CountItem format".to_string())
    } else {
        Ok(CountItem::Pair(inputs[0].clone(), inputs[1].clone()))
    }
}

impl<'de> Deserialize<'de> for KeyCounts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyCountsVisitor;

        impl<'de> Visitor<'de> for KeyCountsVisitor {
            type Value = KeyCounts;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map of CountItem to u32")
            }

            fn visit_map<M>(self, mut map: M) -> Result<KeyCounts, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut counts = HashMap::new();
                let mut config = None;

                while let Some((key_str, value)) = map.next_entry::<String, serde_yaml::Value>()? {
                    match key_str.as_str() {
                        "config" => {
                            config =
                                Some(serde_yaml::from_value(value).map_err(de::Error::custom)?);
                        }
                        _ => {
                            let count_item =
                                parse_count_item(&key_str).map_err(de::Error::custom)?;
                            counts.insert(
                                count_item,
                                value
                                    .as_u64()
                                    .ok_or_else(|| de::Error::custom("Expected u64 value"))?
                                    as u32,
                            );
                        }
                    }
                }

                let config = config.unwrap_or_default();
                Ok(KeyCounts {
                    config,
                    map: counts,
                })
            }
        }
        deserializer.deserialize_map(KeyCountsVisitor)
    }
}

impl Deref for KeyCounts {
    type Target = HashMap<CountItem, u32>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for KeyCounts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

fn main() {
    env_logger::init();

    let mut key_counts = KeyCounts {
        config: None,
        map: HashMap::new(),
    };

    let mut sensitivity = PRODUCTIVE_SENSITIVITY_VALUE;
    let mut force_modify_output = false;
    let mut force_modify_trace = false;
    let mut no_output = false;
    let mut verbose = false;
    let mut no_chords = false;
    let mut statistic_path: Option<PathBuf> = None;
    let mut trace_path: Option<PathBuf> = None;
    let mut pairs = false;
    let mut trace_plain_style = false;
    let mut first_trace_step = true;

    let mut args = env::args();
    let program_name = args.next().expect("this panic not posible");

    while let Some(arg) = args.next() {
        match arg.as_str() {
            SENSITIVITY_SHORT | SENSITIVITY_LONG => {
                sensitivity = match args
                    .next()
                    .or_else(|| {
                        println!(
                            "{} {arg} {}",
                            "error: necessary value for option".red(),
                            "(sensitivity)".red()
                        );
                        std::process::exit(1);
                    })
                    .unwrap()
                    .as_str()
                {
                    PRODUCTIVE_SENSITIVITY_KEY => PRODUCTIVE_SENSITIVITY_VALUE,
                    INTENT_SENSITIVITY_KEY => INTENT_SENSITIVITY_VALUE,
                    other => other
                        .parse::<u64>()
                        .or_else(|_| -> Result<_, ()> {
                            println!(
                                "{} {other} {}\n{} number > 0 {or} {PRODUCTIVE_SENSITIVITY_KEY} {or} {INTENT_SENSITIVITY_KEY}",
                                "error:".red(),
                                "is not valid value for sensitivity".red(),
                                "must be a".red(),
                                or = "or".red(),
                            );
                            std::process::exit(1);
                        })
                        .unwrap(),
                }
            }
            VERSION_SHORT | VERSION_LONG => {
                println!("{}", VERSION);
                std::process::exit(0);
            }
            MODIFY_OUTPUT_SHORT | MODIFY_OUTPUT_LONG => {
                force_modify_output = true;
            }
            PAIRS_SHORT | PAIRS_LONG => pairs = true,
            OUTPUT_SHORT | OUTPUT_LONG => {
                let path = args
                    .next()
                    .or_else(|| {
                        println!(
                            "{} {arg} {}",
                            "error: necessary value for option".red(),
                            "(output)".red()
                        );
                        std::process::exit(1);
                    })
                    .unwrap();

                let path = Path::new(&path);
                statistic_path = Some(path.to_path_buf());
            }
            MODIFY_TRACE_SHORT | MODIFY_TRACE_LONG => {
                force_modify_trace = true;
            }
            TRACE_SHORT | TRACE_LONG => {
                let path = args
                    .next()
                    .or_else(|| {
                        println!(
                            "{} {arg} {}",
                            "error: necessary value for option".red(),
                            "(trace)".red()
                        );
                        std::process::exit(1);
                    })
                    .unwrap();

                let path = Path::new(&path);
                trace_path = Some(path.to_path_buf());
            }
            NO_CHORDS_LONG => no_chords = true,
            NO_OUTPUT_LONG => no_output = true,
            PLAIN_SHORT | PLAIN_LONG => trace_plain_style = true,
            VERBOSE_SHORT | VERBOSE_LONG => verbose = true,
            HELP_SHORT | "-?" | "?" | "h" | HELP_LONG | "-help" | "help" => {
                println!(
                    r#"Program for capture statistic of you keyboard usage

{usage_title} {usage_content}

{optiongs_title}
    {sensitivity_short}, {sensitivity_long} {sensitivity_value}
                    Interprets how often keyboard input will be taken (milliseconds)
                    for reduce CPU usage

                    100 - for a typical keyboard it will be enough
                    1 - very sensitive
                    
                    If your keyboard can handle different modes of keystrokes, 
                    it is possible that some inputs will be too short to not be picked up.
                    
                    It may be worth checking whether all the keys are picked up by the 
                    program and if some cannot be picked up, reduce this value

                    {default} {PRODUCTIVE_SENSITIVITY_VALUE}

    {pairs_short}, {pairs_long} 
                    Save buttons pairs counts instead single

    {no_chords_long}
                    Get inputs separately not paying attention to simultaneous presses
                    
    {modify_output_short}, {modify_output_long}         
                    Force modify output file if it already exists

    {output_short}, {output_long} {output_value}         
                    Output file

                    {default} {DEFAULT_STATISTIC_PATH_YAML}

    {no_output_long}
                    Does not create an output file.
                    Do no effect on trace file ({trace_short}, {trace_long})

    {trace_short}, {trace_long} {trace_value}
                    Save trace (Key, Duratin) in file
                    where Duration is time between curent and last key pressed

    {modify_trace_short}, {modify_trace_long}
                    Force modify trace file if it already exists

    {version_short}, {version_long}         
                    Show the version

    {verbose_short}, {verbose_long}         
                    Describe the steps of the program

    {plain_short}, {plain_long}
                    Trace output in postscript plain style.

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
                    modify_output_short = MODIFY_OUTPUT_SHORT.cyan(),
                    modify_output_long = MODIFY_OUTPUT_LONG.cyan(),
                    output_short = OUTPUT_SHORT.cyan(),
                    output_long = OUTPUT_LONG.cyan(),
                    output_value = "<path>".cyan(),
                    default = "Default:".green(),
                    trace_short = TRACE_SHORT.cyan(),
                    trace_long = TRACE_LONG.cyan(),
                    trace_value = "<path>".cyan(),
                    pairs_short = PAIRS_SHORT.cyan(),
                    pairs_long = PAIRS_LONG.cyan(),
                    no_chords_long = NO_CHORDS_LONG.cyan(),
                    modify_trace_short = MODIFY_TRACE_SHORT.cyan(),
                    modify_trace_long = MODIFY_TRACE_LONG.cyan(),
                    plain_short = PLAIN_SHORT.cyan(),
                    plain_long = PLAIN_LONG.cyan(),
                    no_output_long = NO_OUTPUT_LONG.cyan(),
                );

                std::process::exit(0);
            }
            _ => {
                println!("Unhandled option: {}", arg);
                std::process::exit(1);
            }
        }
    }

    if trace_plain_style && trace_path.is_none() {
        println!(
            "{warning} {PLAIN_SHORT} {or} {PLAIN_LONG} {text}{TRACE_SHORT} {pipe} {TRACE_LONG}{brace}",
            warning = "warning!:".yellow(),
            or = "or".yellow(),
            text = "ignored becouse you do not specified trace option (".yellow(),
            pipe = "|".yellow(),
            brace = ")".yellow(),
        );
    }

    if force_modify_trace && trace_path.is_none() {
        println!(
            "{warning} {MODIFY_TRACE_SHORT} {or} {MODIFY_TRACE_LONG} {text}{TRACE_SHORT} {pipe} {TRACE_LONG}{brace}",
            warning = "warning!:".yellow(),
            or = "or".yellow(),
            text = "ignored becouse you do not specified trace option (".yellow(),
            pipe = "|".yellow(),
            brace = ")".yellow(),
        );
    }

    if force_modify_output && no_output {
        println!(
            "{warning} {MODIFY_OUTPUT_SHORT} {or} {MODIFY_OUTPUT_LONG} {text}{NO_OUTPUT_LONG}{brace}",
            warning = "warning!:".yellow(),
            or = "or".yellow(),
            text = "ignored becouse you do specified no_output option (".yellow(),
            brace = ")".yellow(),
        );
    }

    if statistic_path.is_some() && no_output {
        println!(
            "{warning} {OUTPUT_SHORT} {or} {OUTPUT_LONG} {text}{NO_OUTPUT_LONG}{brace}",
            warning = "warning!:".yellow(),
            or = "or".yellow(),
            text = "ignored becouse you do specified no_output option (".yellow(),
            brace = ")".yellow(),
        );
    }

    // process the output file
    if !no_output {
        if statistic_path == None {
            statistic_path = Some(Path::new(DEFAULT_STATISTIC_PATH_YAML).to_path_buf());
        }
        let path = statistic_path.as_ref().unwrap();

        if path.exists() {
            let mut file = File::open(&path)
                .or_else(|_| -> Result<_, ()> {
                    println!(
                        "{} {} {}",
                        "file in".red(),
                        path.to_string(),
                        "exists but cannot be open".red()
                    );
                    std::process::exit(1);
                })
                .unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .or_else(|_| -> Result<_, ()> {
                    println!(
                        "{} {} {}",
                        "file in".red(),
                        path.to_string(),
                        "exists but cannot be read".red()
                    );
                    std::process::exit(1);
                })
                .unwrap();

            key_counts = serde_yaml::from_str(&contents)
                .or_else(|_| -> Result<_, ()> {
                    println!(
                        "{} {} {}",
                        "error: data in output file".red(),
                        path.to_string(),
                        "not valid and cannot be deserialize".red()
                    );
                    std::process::exit(1);
                })
                .unwrap();

            ask_modify(force_modify_output, path);
        }
    }

    if let Some(ref mut config) = key_counts.config {
        check_config(
            config,
            force_modify_output,
            pairs,
            no_chords,
            statistic_path.as_ref().unwrap(),
        );
    } else {
        key_counts.config = Some(Config::new(pairs, no_chords, VERSION.into()));
    }

    // save first time to check open/write errors
    save_data(&key_counts, statistic_path.as_ref().unwrap(), no_output);
    if let Some(ref trace_path) = trace_path {
        if trace_path.exists() {
            ask_modify(force_modify_trace, trace_path);
        }
        upend_trace(TraceStep::Empty, &trace_path, trace_plain_style);
    }

    let device_state = DeviceState::new();
    let mut last_keys = Vec::new();
    let mut last_pair = Vec::new();

    let mut stdout = stdout();

    if verbose {
        enable_raw_mode().expect("enable_raw_mode problem");
        execute!(stdout, EnterAlternateScreen).expect("EnterAlternateScreen problem");
    }

    let start = Instant::now();
    let mut last_duration = start.elapsed();
    let mut some = false;

    loop {
        let keys = device_state.get_keys();

        // Check for new key presses
        for key in &keys {
            if !last_keys.contains(key) {
                some = true;
                if no_chords {
                    if pairs {
                        // skip first iteration becouse it is have not pair
                        if last_pair.len() != 0 {
                            let count_item = CountItem::Pair(last_pair, vec![*key]);
                            *key_counts.entry(count_item.clone()).or_insert(0) += 1;

                            verbose!(
                                verbose,
                                "{:?} has been pressed {} times",
                                count_item,
                                key_counts[&count_item]
                            );

                            // Is this expensive?)
                            save_data(&key_counts, statistic_path.as_ref().unwrap(), no_output);
                        }

                        last_pair = vec![*key];
                    } else {
                        let count_item = CountItem::Single(vec![*key]);
                        *key_counts.entry(count_item.clone()).or_insert(0) += 1;
                        verbose!(
                            verbose,
                            "{:?} has been pressed {} times",
                            *key,
                            key_counts[&count_item]
                        );

                        // Is this expensive?)
                        save_data(&key_counts, statistic_path.as_ref().unwrap(), no_output);
                    }

                    if let Some(ref trace_path) = trace_path {
                        let duration = start.elapsed();
                        let step = if first_trace_step {
                            first_trace_step = false;
                            TraceStep::First(vec![*key])
                        } else {
                            TraceStep::Regular(vec![*key], duration - last_duration)
                        };

                        last_duration = duration;

                        upend_trace(step, &trace_path, trace_plain_style);
                    }
                }
            }
        }

        if some {
            if !no_chords {
                if pairs {
                    // skip first iteration becouse it is have not pair
                    if last_pair.len() != 0 {
                        let count_item = CountItem::Pair(last_pair, keys.clone());
                        *key_counts.entry(count_item.clone()).or_insert(0) += 1;
                        verbose!(
                            verbose,
                            "{:?} has been pressed {} times",
                            count_item,
                            key_counts[&count_item]
                        );

                        // Is this expensive?)
                        save_data(&key_counts, statistic_path.as_ref().unwrap(), no_output);
                    }

                    last_pair = keys.clone();
                } else {
                    let count_item = CountItem::Single(keys.clone());
                    *key_counts.entry(count_item.clone()).or_insert(0) += 1;
                    verbose!(
                        verbose,
                        "{:?} has been pressed {} times",
                        keys.clone(),
                        key_counts[&count_item]
                    );

                    // Is this expensive?)
                    save_data(&key_counts, statistic_path.as_ref().unwrap(), no_output);
                }

                if let Some(ref trace_path) = trace_path {
                    let duration = start.elapsed();
                    let step = if first_trace_step {
                        first_trace_step = false;
                        TraceStep::First(keys.clone())
                    } else {
                        TraceStep::Regular(keys.clone(), duration - last_duration)
                    };

                    last_duration = duration;

                    upend_trace(step, &trace_path, trace_plain_style);
                }
            }
            some = false;
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
        } else {
            thread::sleep(Duration::from_millis(sensitivity));
        }
    }

    if verbose {
        execute!(stdout, LeaveAlternateScreen).expect("LeaveAlternateScreen problem");
        disable_raw_mode().expect("disable_raw_mode problem");
    }
}

fn save_data(data: &KeyCounts, path: &PathBuf, no_output: bool) {
    if no_output {
        return;
    }
    let serialized = serde_yaml::to_string(data).expect("serialize to yaml panic");
    let mut file = File::create(path)
        .or_else(|_| -> Result<_, ()> {
            println!("{} {}", "cannot create / open file".red(), path.to_string(),);
            std::process::exit(1);
        })
        .unwrap();
    file.write_all(serialized.as_bytes())
        .or_else(|_| -> Result<_, ()> {
            println!("{} {}", "cannot write to file".red(), path.to_string(),);
            std::process::exit(1);
        })
        .unwrap();
}

fn upend_trace(trace_step: TraceStep, path: &PathBuf, trace_plain_style: bool) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .or_else(|_| -> Result<_, ()> {
            println!("{} {}", "cannot create / open file".red(), path.to_string(),);
            std::process::exit(1);
        })
        .unwrap();

    let text = match trace_step {
        TraceStep::First(keycode) => input_to_string(keycode),
        TraceStep::Regular(keycode, duration) => {
            if trace_plain_style {
                input_to_string(keycode)
            } else {
                format!("{} after {:?}", input_to_string(keycode), duration).replace("\"", "")
            }
        }
        TraceStep::Empty => return,
    };

    if trace_plain_style {
        write!(file, "{} ", text)
            .or_else(|_| -> Result<_, ()> {
                println!("{} {}", "cannot write to file ".red(), path.to_string(),);
                std::process::exit(1);
            })
            .unwrap();
    } else {
        write!(file, "{}\n", text)
            .or_else(|_| -> Result<_, ()> {
                println!("{} {}", "cannot write to file ".red(), path.to_string(),);
                std::process::exit(1);
            })
            .unwrap();
    }
}

fn check_config(
    config: &mut Config,
    force_modify_output: bool,
    pairs: bool,
    no_chords: bool,
    path: &PathBuf,
) {
    let error = format!(
        "Config in output file that you provide {:?} do not match to your options\nit is means different settings were used to create this file\n",
        path
    ).red();
    if config.pairs != pairs {
        println!(
            "{error}{details_title} {pairs_short} {or} {pairs_long} {is} {pairs} {when_in_file} {config_pairs}",
            details_title="Details:".red(),
            pairs_short = PAIRS_SHORT.cyan(),
            or="or".red(),
            pairs_long=PAIRS_LONG.cyan(),
            is="is".red(),
            pairs=pairs.to_string().cyan(),
            when_in_file="when in file".red(),
            config_pairs = config.pairs.to_string().cyan(),
        );
        std::process::exit(1);
    }

    if config.no_chords != no_chords {
        println!(
            "{error}{details_title} {no_chords_long} {is} {no_chords} {when_in_file} {config_no_chords}",
            details_title="Details:".red(),
            no_chords_long = NO_CHORDS_LONG.cyan(),
            is="is".red(),
            no_chords=no_chords.to_string().cyan(),
            when_in_file="when in file".red(),
            config_no_chords = config.no_chords.to_string().cyan(),
        );
        std::process::exit(1);
    }

    if config.version != VERSION {
        println!(
            "{warning} {config_verison}{curent_is} {VERSION}",
            warning = "warning!: the output file last updated on".yellow(),
            config_verison = config.version,
            curent_is = ", curent version is".yellow(),
        );

        if !force_modify_output {
            print!("are you sure you want to modify this file? [y/N] ");
            io::stdout().flush().unwrap();
            let mut buffer = [0; 1];
            io::stdin()
                .read_exact(&mut buffer)
                .or_else(|_| -> Result<_, ()> {
                    println!("{}", "cannot read terminal input".red());
                    std::process::exit(1);
                })
                .unwrap();
            let character = buffer[0] as char;

            match character {
                'y' | 'Y' => {}
                _ => std::process::exit(0),
            }
        }
        config.version = VERSION.into();
    }
}

fn ask_modify(force: bool, path: &PathBuf) {
    if !force {
        println!(
            "{} ({:?}) {}",
            "warning!: file that you provide like output".yellow(),
            path,
            "already exist".yellow(),
        );
        print!("are you sure you want to modify this file? [y/N] ");
        io::stdout().flush().unwrap();

        let mut buffer = [0; 1];
        io::stdin()
            .read_exact(&mut buffer)
            .or_else(|_| -> Result<_, ()> {
                println!("{}", "cannot read terminal input".red());
                std::process::exit(1);
            })
            .unwrap();
        let character = buffer[0] as char;

        match character {
            'y' | 'Y' => {}
            _ => std::process::exit(0),
        }
    }
}
