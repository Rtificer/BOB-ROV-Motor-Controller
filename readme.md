# Structure
This repo is a workspace which contains both the rp2040-dshot library and the motor controller package for Sunk Robotics upcoming BOB ROV.

## Building and Running
To build for release use:\
`cargo build --release --package rp2040-dshot --target thumbv6m-none-eabi`\
or\
`cargo build --release --package motor-controller --target thumbv6m-none-eabi`,\
respectively


To run, flash the left pico with picoprobe, by holding down the bootsel button while plugging it in, then moving the file into its flash storage.\
Then set up a picoprobe according to the following wiring diagram:\
<img src="assets/picoprobe wiring.jpg" width="384">\
Then run using:\
`cargo run --release --package motor-controller --target thumbv6m-none-eabi`\
This should automatically flash the target pico with the motor-controller code. You should be able to read erros and warnings through the pico probe.\

To run tests simply run:\
`cargo test`

## Configuration
In [motor-controller/src/config.rs](motor-controller/src/config.rs) the configuration is split between i2c, dshot, and telmetry. Simply edit the values where the define_{telemetry_type}_config! macros are called. Example:
```rust
define_i2c_config! {
    peripheral: I2C0,
    scl_pin: PIN_1,
    sda_pin: PIN_0,
    addr: 0x60,
    general_call: false,
    scl_pullup: false,
    sda_pullup: false,
    buffer_size: 128
}
```

## TODO
- Finish Documentation
- Get tests working on non-embedded os
- Finish writing tests for both library and motor controller code