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
use libtock_core::syscalls;
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

libtock_core::stack_size! {0x2000}

fn print_packet(pkt: &[u8]) {
    let mut console = Console::new();
    write!(console, "[ ").unwrap();
    for byte in pkt {
        write!(console, "{:02X} ", byte).unwrap();
    }
    writeln!(console, "]").unwrap();
    console.flush();
}

fn main() {
    let mut console = Console::new();

    let mem_start = unsafe { syscalls::raw::memop(2, 0) };
    let mem_end = unsafe { syscalls::raw::memop(3, 0) };

    writeln!(console, "Memory start: {:#08X}", mem_start).unwrap();
    writeln!(console, "Memory end: {:#08X}", mem_end).unwrap();
    writeln!(console, "Memory size: {:#08X}", mem_end - mem_start).unwrap();
    console.flush();

    // Setup the timer with a dummy callback (we only care about reading the current time, but the
    // API forces us to set an alarm callback too).
    let mut with_callback = timer::with_callback(|_, _| {});
    let timer = with_callback.init().flex_unwrap();

    // Setup USB driver.
    if !usb_ctap_hid::setup() {
        panic!("Cannot setup USB driver");
    }

    writeln!(console, "Successfully setup USB driver").unwrap();
    console.flush();

    let boot_time = timer::ClockValue::new(0, 0);

    let mut rng = TockRng256 {};
    let mut ctap_state = CtapState::new(&mut rng, check_user_presence, boot_time);
    let mut ctap_hid = CtapHid::new();

    // Main loop. If CTAP1 is used, we register button presses for U2F while receiving and waiting.
    // The way TockOS and apps currently interact, callbacks need a yield syscall to execute,
    // making consistent blinking patterns and sending keepalives harder.
    loop {
        writeln!(console, "Receiving packet from USB").unwrap();
        console.flush();

        let mut pkt_request = [0; 64];
        let has_packet = usb_ctap_hid::recv(&mut pkt_request);

        if has_packet {
            write!(console, "Received packet from USB: ").unwrap();
            console.flush();
            print_packet(&pkt_request);
        } else {
            panic!("Error receiving packet");
        }

        let now = timer::ClockValue::new(1, 1);
        if has_packet {
            writeln!(console, "Processing packet response").unwrap();
            console.flush();
            let mut reply = ctap_hid.process_hid_packet(&pkt_request, now, &mut ctap_state);
            writeln!(console, "Processed packet response").unwrap();
            console.flush();
            // This block handles sending packets.
            for mut pkt_reply in reply {
                let sent = usb_ctap_hid::send(&mut pkt_reply);
                if sent {
                    write!(console, "Sent response packet to USB: ").unwrap();
                    console.flush();
                    print_packet(&pkt_reply);
                } else {
                    panic!("Error sending packet");
                }
            }
        }
    }
}

fn check_user_presence(cid: ChannelID) -> Result<(), Ctap2StatusCode> {
    Ok(())
}
