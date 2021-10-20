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

use crate::console::Console;
use crate::result::TockError;
use crate::timer;
use crate::timer::Duration;
use crate::util;
use core::cell::Cell;
use core::fmt::Write;
use libtock_core::result::{CommandError, EALREADY, EBUSY, SUCCESS};
use libtock_core::{callback, syscalls};

const DRIVER_NUMBER: usize = 0x20008;

mod command_nr {
    pub const CHECK: usize = 0;
    pub const TRANSMIT: usize = 1;
    pub const RECEIVE: usize = 2;
}

mod subscribe_nr {
    pub const TRANSMIT: usize = 1;
    pub const RECEIVE: usize = 2;
}

mod allow_nr {
    pub const TRANSMIT: usize = 1;
    pub const RECEIVE: usize = 2;
}

pub fn setup() -> bool {
    let result = syscalls::command(DRIVER_NUMBER, command_nr::CHECK, 0, 0);
    if result.is_err() {
        return false;
    }

    true
}

#[allow(dead_code)]
pub fn recv(buf: &mut [u8; 64]) -> bool {
    let result = syscalls::allow(DRIVER_NUMBER, allow_nr::RECEIVE, buf);
    if result.is_err() {
        return false;
    }

    let done = Cell::new(false);
    let mut alarm = || done.set(true);
    let subscription = syscalls::subscribe::<callback::Identity0Consumer, _>(
        DRIVER_NUMBER,
        subscribe_nr::RECEIVE,
        &mut alarm,
    );
    if subscription.is_err() {
        return false;
    }

    let result_code = syscalls::command(DRIVER_NUMBER, command_nr::RECEIVE, 0, 0);
    if result_code.is_err() {
        return false;
    }

    util::yieldk_for(|| done.get());
    true
}

#[allow(dead_code)]
pub fn send(buf: &mut [u8; 64]) -> bool {
    let result = syscalls::allow(DRIVER_NUMBER, allow_nr::TRANSMIT, buf);
    if result.is_err() {
        return false;
    }

    let done = Cell::new(false);
    let mut alarm = || done.set(true);
    let subscription = syscalls::subscribe::<callback::Identity0Consumer, _>(
        DRIVER_NUMBER,
        subscribe_nr::TRANSMIT,
        &mut alarm,
    );
    if subscription.is_err() {
        return false;
    }

    let result_code = syscalls::command(DRIVER_NUMBER, command_nr::TRANSMIT, 0, 0);
    if result_code.is_err() {
        return false;
    }

    util::yieldk_for(|| done.get());
    true
}

// Same as recv, but with a timeout.
// If the timeout elapses, return None.
#[allow(clippy::let_and_return)]
pub fn recv_with_timeout(buf: &mut [u8; 64], timeout_delay: Duration<isize>) -> bool {
    writeln!(
        Console::new(),
        "Receiving packet with timeout of {}ms",
        timeout_delay.ms(),
    )
    .unwrap();

    let result = recv_with_timeout_detail(buf, timeout_delay);

    {
        if result {
            writeln!(Console::new(), "Received packet = {:02x?}", buf as &[u8]).unwrap();
        }
    }

    result
}

fn recv_with_timeout_detail(buf: &mut [u8; 64], timeout_delay: Duration<isize>) -> bool {
    let result = syscalls::allow(DRIVER_NUMBER, allow_nr::RECEIVE, buf);
    if result.is_err() {
        return false;
    }

    let done = Cell::new(false);
    let mut alarm = || done.set(true);
    let subscription = syscalls::subscribe::<callback::Identity0Consumer, _>(
        DRIVER_NUMBER,
        subscribe_nr::RECEIVE,
        &mut alarm,
    );
    if subscription.is_err() {
        return false;
    }

    // Setup a time-out callback.
    let timeout_expired = Cell::new(false);
    let mut timeout_callback = timer::with_callback(|_, _| {
        timeout_expired.set(true);
    });
    let mut timeout = match timeout_callback.init() {
        Ok(x) => x,
        Err(_) => return false,
    };
    let timeout_alarm = match timeout.set_alarm(timeout_delay) {
        Ok(x) => x,
        Err(_) => return false,
    };

    // Trigger USB reception.
    let result_code = syscalls::command(DRIVER_NUMBER, command_nr::RECEIVE, 0, 0);
    if result_code.is_err() {
        return false;
    }

    util::yieldk_for(|| done.get() || timeout_expired.get());

    // Cleanup alarm callback.
    match timeout.stop_alarm(timeout_alarm) {
        Ok(()) => (),
        Err(TockError::Command(CommandError {
            return_code: EALREADY,
            ..
        })) => {
            if !timeout_expired.get() {
                #[cfg(feature = "debug_ctap")]
                writeln!(
                    Console::new(),
                    "The receive timeout already expired, but the callback wasn't executed."
                )
                .unwrap();
            }
        }
        Err(_e) => {
            #[cfg(feature = "debug_ctap")]
            panic!("Unexpected error when stopping alarm: {:?}", _e);
            #[cfg(not(feature = "debug_ctap"))]
            panic!("Unexpected error when stopping alarm: <error is only visible with the debug_ctap feature>");
        }
    }

    done.get()
}
