# mff-hr-v1

[![Continuous Integration](https://github.com/nullstalgia/mff-hr-v1/actions/workflows/rust_ci.yml/badge.svg)](https://github.com/nullstalgia/mff-hr-v1/actions/workflows/rust_ci.yml)

### Demo:

![2024-12-24--19-23-36-msrdc](https://github.com/user-attachments/assets/023fd233-3d67-4a81-8f27-831d802a7d02)

Myself and a group of friends went to MFF together, and I slapped together a firmware for the 2-USB-Port variant of the Cheap Yellow Display for us to wear.

The badge lets you calibrate the touch input, enter a username, load QOI-format images from an SD Card in a configurable slideshow, and show the user's heartrate from any BLE Heart Rate Monitor!

This was a bit of a smoke test for using Rust in a constrained environment (only three weeks, have to be able to quickly move to my laptop/a friend's machine, and under 1MB of RAM on a base ESP32). As such, the code isn't the most beautiful or efficient, but it worked reliably for the whole con with battery to spare each day.

In that short time, I was able to set up:

- [esp-idf-hal](https://github.com/esp-rs/esp-idf-hal), which has been largely pleasant to work with, including on GitHub CI!
- An updated fork of a driver for Touchscreen IC I'd never used, copying calibration math from an Arduino library
- An updated fork of bitbang-hal for SPI to then use ^ this crate
- A UI to input names of variable lengths
- A UI to select nearby BLE Heart Rate Monitors (both without any UI crates!)
- A reactive history line for the incoming BPM

And more that isn't as impressive, but given the short time frame for the whole ordeal, I'm very pleased.

Granted, I wouldn't consider my updated forks to be exhaustive and ready to send in as PRs, but I would like to formalize my changes and get them upstreamed.

What I didn't get to do this year:

- I really wanted to load GIFs! I still think it's possible to stream them in from the SD to the display (no idea how buffering would work yet), since trying to keep a whole GIF, let alone a full frame, in memory is a challenge.
- I wanted to have a more intricate effect on the name and it's letters, but I'm still happy with the friendly bounce.
- I'd have also liked to use more of the HR logic that comes from [iron-heart](https://github.com/nullstalgia/iron-heart) for visual effects.
- HR Data logging to SD! I could've thrown together a quick serializer but I was concerned there not being a way to timestamp them very well.

This could not have been possible without the works of:
- The entire esp-rs team, love y'all.
- embedded-hal/graphics/and more.
- https://github.com/ardnew/XPT2046_Calibrated/blob/8d3f8b518b617b6fbc870ef3229b27aa83028c56/src/XPT2046_Calibrated.cpp
- https://github.com/arashsm79/OFMon/blob/afca7d019f3e7efe79879b72dbd4a7d22d660c2d/src/main.rs

What's next?

Stay tuned!
