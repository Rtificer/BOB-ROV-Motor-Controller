# Structure
This repo is a workspace which contains both the rp2040-dshot library and the motor controller package for Sunk Robotics upcoming BOB ROV.

## Building and Running
To build for release use:\
`cargo build --release --package rp2040-dshot --target thumbv6m-none-eabi`\
or\
`cargo build --release --package motor-controller --target thumbv6m-none-eabi`,\
respectively


To run, set up a picoprobe according to the following wiring diagram:\
<img src="assets/picoprobe wiring.jpg" width="300">
Then run:\
`cargo run --release --package motor-controller --target thumbv6m-none-eabi`

To run tests simply run:\
`cargo test`

## TODO
- Finish Documentation
- Get tests working on non-embedded os
- Finish writing tests for both library and motor controller code