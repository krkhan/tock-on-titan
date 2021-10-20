// Copyright 2019 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate core;
#[macro_use]
extern crate arrayref;
extern crate byteorder;

mod ctap;

use core::cell::Cell;
use core::fmt::Write;
use crypto::rng256::TockRng256;
use ctap::hid::{ChannelID, CtapHid, KeepaliveStatus, ProcessedPacket};
use ctap::status_code::Ctap2StatusCode;
use ctap::CtapState;
use libtock_core::result::{CommandError, EALREADY};
use libtock_drivers::buttons;
use libtock_drivers::buttons::ButtonState;
use libtock_drivers::console::Console;
use libtock_drivers::led;
use libtock_drivers::result::{FlexUnwrap, TockError};
use libtock_drivers::timer;
use libtock_drivers::timer::Duration;
#[cfg(feature = "debug_ctap")]
use libtock_drivers::timer::Timer;
#[cfg(feature = "debug_ctap")]
use libtock_drivers::timer::Timestamp;
use libtock_drivers::usb_ctap_hid;

const KEEPALIVE_DELAY_MS: isize = 100;
const KEEPALIVE_DELAY: Duration<isize> = Duration::from_ms(KEEPALIVE_DELAY_MS);
const SEND_TIMEOUT: Duration<isize> = Duration::from_ms(1000);

libtock_core::stack_size! {0x2000}

fn main() {
    // Setup the timer with a dummy callback (we only care about reading the current time, but the
    // API forces us to set an alarm callback too).
    let mut with_callback = timer::with_callback(|_, _| {});
    let timer = with_callback.init().flex_unwrap();
    let mut console = Console::new();

    // Setup USB driver.
    if !usb_ctap_hid::setup() {
        panic!("Cannot setup USB driver");
    }

    writeln!(console, "Successfully setup USB driver").unwrap();
    console.flush();

    let boot_time = timer.get_current_clock().flex_unwrap();
    let mut rng = TockRng256 {};
    let mut ctap_state = CtapState::new(&mut rng, check_user_presence, boot_time);
    let mut ctap_hid = CtapHid::new();

    // Main loop. If CTAP1 is used, we register button presses for U2F while receiving and waiting.
    // The way TockOS and apps currently interact, callbacks need a yield syscall to execute,
    // making consistent blinking patterns and sending keepalives harder.
    loop {
        let mut pkt_request = [0; 64];
        let has_packet = usb_ctap_hid::recv_with_timeout(&mut pkt_request, KEEPALIVE_DELAY);

        if has_packet {
            #[cfg(feature = "debug_ctap")]
            print_packet_notice("Received packet", &timer);
        } else {
            panic!("Error receiving packet");
        }

        let now = timer.get_current_clock().flex_unwrap();

        // These calls are making sure that even for long inactivity, wrapping clock values
        // don't cause problems with timers.
        ctap_state.update_timeouts(now);
        ctap_hid.wink_permission = ctap_hid.wink_permission.check_expiration(now);

        if has_packet {
            let reply = ctap_hid.process_hid_packet(&pkt_request, now, &mut ctap_state);
            // This block handles sending packets.
            for mut pkt_reply in reply {
                let sent = usb_ctap_hid::send(&mut pkt_reply);
                if sent {
                    #[cfg(feature = "debug_ctap")]
                    print_packet_notice("Sent packet", &timer);
                } else {
                    panic!("Error sending packet");
                }
            }
        }

        let now = timer.get_current_clock().flex_unwrap();
    }
}

#[cfg(feature = "debug_ctap")]
fn print_packet_notice(notice_text: &str, timer: &Timer) {
    let now = timer.get_current_clock().flex_unwrap();
    let now_us = (Timestamp::<f64>::from_clock_value(now).ms() * 1000.0) as u64;
    writeln!(
        Console::new(),
        "{} at {}.{:06} s",
        notice_text,
        now_us / 1_000_000,
        now_us % 1_000_000
    )
    .unwrap();
}

fn blink_leds(pattern_seed: usize) {
    for l in 0..led::count().flex_unwrap() {
        if (pattern_seed ^ l).count_ones() & 1 != 0 {
            led::get(l).flex_unwrap().on().flex_unwrap();
        } else {
            led::get(l).flex_unwrap().off().flex_unwrap();
        }
    }
}

fn wink_leds(pattern_seed: usize) {
    // This generates a "snake" pattern circling through the LEDs.
    // Fox example with 4 LEDs the sequence of lit LEDs will be the following.
    // 0 1 2 3
    // * *
    // * * *
    //   * *
    //   * * *
    //     * *
    // *   * *
    // *     *
    // * *   *
    // * *
    let count = led::count().flex_unwrap();
    let a = (pattern_seed / 2) % count;
    let b = ((pattern_seed + 1) / 2) % count;
    let c = ((pattern_seed + 3) / 2) % count;

    for l in 0..count {
        // On nRF52840-DK, logically swap LEDs 3 and 4 so that the order of LEDs form a circle.
        let k = match l {
            2 => 3,
            3 => 2,
            _ => l,
        };
        if k == a || k == b || k == c {
            led::get(l).flex_unwrap().on().flex_unwrap();
        } else {
            led::get(l).flex_unwrap().off().flex_unwrap();
        }
    }
}

fn switch_off_leds() {
    for l in 0..led::count().flex_unwrap() {
        led::get(l).flex_unwrap().off().flex_unwrap();
    }
}

fn check_user_presence(cid: ChannelID) -> Result<(), Ctap2StatusCode> {
    Ok(())
}
