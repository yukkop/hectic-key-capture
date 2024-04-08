# Hectic Key Capture

Hectic Key Capture is a tool designed to capture keyboard usage statistics. I use this program for collecting data to train AI models aimed at finding the optimal keyboard layout.

## Installation

### From sources

To install `hectic-key-capture` from sources, ensure you have Rust and Cargo installed on your system. Then, follow these steps:

```sh
git clone https://github.com/<your-username>/hectic-key-capture.git
cd hectic-key-capture
cargo build --release
```

`./target/release/hectic-key-capture` - bin file that you can use

## Usage
To run `hectic-key-capture`, use the following command:

```sh
hectic-key-capture [OPTIONS]
```

### Options

 - `-s`, `--sensitivity`: Set the sensitivity for keyboard input capture. Defaults to 100 milliseconds.
 - `-y`, `--modify-output`: Force modification of the existing output file.
 - `-o`, `--output <path>`: Specify the output file path. Defaults to `key-capture-statistic.yaml`.
 - `-t`, `--trace <path>`: Save a trace of key presses and durations to a file.
 - `-V`, `--version`: Display the program version.
 - `-v`, `--verbose`: Enable verbose output.
 - `-h`, `--help`: Show the help message.

## Example
To run the program with a sensitivity of 100ms and verbose output, saving the statistics to a specified file:

```sh
cargo run -- -s 50 -v -o my-stats.yaml
```

## License
[LICENSE](LICENSE)
