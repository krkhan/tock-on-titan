// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// NOTE: The code uses asserts and expect to ease debugging.

use core::cmp;
use core::convert::TryFrom;
use h1::hil::flash::{Client, Flash};
use kernel::common::cells::OptionalCell;
use kernel::{AppId, AppSlice, Callback, Driver, Grant, ReturnCode, Shared};

pub const DRIVER_NUM: usize = 0x50003;

type WORD = u32;
const WORD_SIZE: usize = core::mem::size_of::<WORD>();
const PAGE_SIZE: usize = 2048;
const FLASH_START: usize = 0x40000;
const MAX_WORD_WRITES: usize = 2;
const MAX_PAGE_ERASES: usize = 10000;
const MAX_WRITE_LENGTH: usize = 32;
const WORD_MASK: usize = WORD_SIZE - 1;
const PAGE_MASK: usize = PAGE_SIZE - 1;

// For some reason, writes seem to fail when spaning a 256 byte boundary.
const WEIRD_SIZE: usize = 64; // words

// To avoid allocating in the kernel, we use this static buffer.
static mut WRITE_BUFFER: [WORD; MAX_WRITE_LENGTH] = [0; MAX_WRITE_LENGTH];

#[derive(Default)]
pub struct App {
    callback: Option<Callback>,
    slice: Option<AppSlice<Shared, u8>>,
}

struct WriteState {
    ptr: usize, // in words
    slice: AppSlice<Shared, u8>,
    offset: usize, // in words
}

pub struct OpenskSyscall<'c, C: Flash<'c>> {
    flash: &'c C,
    apps: Grant<App>,
    waiting: OptionalCell<AppId>,
    writing: OptionalCell<WriteState>,
}

impl<'c, C: Flash<'c>> OpenskSyscall<'c, C> {
    pub fn new(flash: &'c C, apps: Grant<App>) -> Self {
        OpenskSyscall {
            flash,
            apps,
            waiting: OptionalCell::empty(),
            writing: OptionalCell::empty(),
        }
    }

    fn write_block(&self, mut state: WriteState) -> ReturnCode {
        let max_length = cmp::min(
            WEIRD_SIZE - (state.ptr + state.offset) % WEIRD_SIZE,
            MAX_WRITE_LENGTH,
        );
        let data_length = cmp::min(state.slice.len() / WORD_SIZE - state.offset, max_length);
        let slice = &state.slice.as_ref()[state.offset * WORD_SIZE..];
        let data = unsafe { &mut WRITE_BUFFER[..data_length] };
        for (dst, src) in data.iter_mut().zip(slice.chunks(WORD_SIZE)) {
            // `unwrap` cannot fail because `slice.len()` is word-aligned.
            *dst = WORD::from_ne_bytes(<[u8; WORD_SIZE]>::try_from(src).unwrap());
        }
        let target = state.ptr + state.offset - FLASH_START / WORD_SIZE;
        state.offset += data_length;
        self.writing.set(state);
        self.flash.write(target, data).0
    }

    fn write_slice(&self, ptr: usize, slice: AppSlice<Shared, u8>) -> ReturnCode {
        if ptr < FLASH_START || ptr & WORD_MASK != 0 || slice.len() & WORD_MASK != 0 {
            return ReturnCode::EINVAL;
        }
        self.write_block(WriteState {
            ptr: ptr / WORD_SIZE,
            slice,
            offset: 0,
        })
    }

    fn erase_page(&self, ptr: usize) -> ReturnCode {
        if ptr < FLASH_START || ptr & PAGE_MASK != 0 {
            return ReturnCode::EINVAL;
        }
        let target = (ptr - FLASH_START) / PAGE_SIZE;
        self.flash.erase(target)
    }

    fn done(&self, status: ReturnCode) {
        self.waiting.take().map(|appid| {
            self.apps.enter(appid, |app, _| {
                app.callback.map(|mut cb| {
                    cb.schedule(status.into(), 0, 0);
                });
            })
        });
    }
}

impl<'c, C: Flash<'c>> Driver for OpenskSyscall<'c, C> {
    fn subscribe(
        &self,
        subscribe_num: usize,
        callback: Option<Callback>,
        appid: AppId,
    ) -> ReturnCode {
        match subscribe_num {
            0 => self
                .apps
                .enter(appid, |app, _| {
                    app.callback = callback;
                    ReturnCode::SUCCESS
                })
                .unwrap_or_else(|err| err.into()),
            _ => ReturnCode::ENOSUPPORT,
        }
    }

    fn command(&self, cmd: usize, arg0: usize, arg1: usize, appid: AppId) -> ReturnCode {
        match (cmd, arg0, arg1) {
            (0, _, _) => ReturnCode::SUCCESS,

            (1, 0, _) => ReturnCode::SuccessWithValue { value: WORD_SIZE },
            (1, 1, _) => ReturnCode::SuccessWithValue { value: PAGE_SIZE },
            (1, 2, _) => ReturnCode::SuccessWithValue {
                value: MAX_WORD_WRITES,
            },
            (1, 3, _) => ReturnCode::SuccessWithValue {
                value: MAX_PAGE_ERASES,
            },
            (1, _, _) => ReturnCode::EINVAL,

            (2, ptr, len) => self
                .apps
                .enter(appid, |app, _| {
                    let slice = match app.slice.take() {
                        None => return ReturnCode::EINVAL,
                        Some(slice) => slice,
                    };
                    if len != slice.len() {
                        return ReturnCode::EINVAL;
                    }
                    if self.waiting.is_some() {
                        return ReturnCode::EBUSY;
                    }
                    self.waiting.set(appid);
                    self.write_slice(ptr, slice)
                })
                .unwrap_or_else(|err| err.into()),

            (3, ptr, len) => {
                if len != PAGE_SIZE {
                    return ReturnCode::EINVAL;
                }
                if self.waiting.is_some() {
                    return ReturnCode::EBUSY;
                }
                self.waiting.set(appid);
                self.erase_page(ptr)
            }

            _ => ReturnCode::ENOSUPPORT,
        }
    }

    fn allow(
        &self,
        appid: AppId,
        allow_num: usize,
        slice: Option<AppSlice<Shared, u8>>,
    ) -> ReturnCode {
        match allow_num {
            0 => self
                .apps
                .enter(appid, |app, _| {
                    app.slice = slice;
                    ReturnCode::SUCCESS
                })
                .unwrap_or_else(|err| err.into()),
            _ => ReturnCode::ENOSUPPORT,
        }
    }
}

impl<'c, C: Flash<'c>> Client<'c> for OpenskSyscall<'c, C> {
    fn erase_done(&self, status: ReturnCode) {
        self.done(status);
    }

    fn write_done(&self, _: &'c mut [u32], status: ReturnCode) {
        let state = self.writing.take().unwrap();
        if status != ReturnCode::SUCCESS || state.offset == state.slice.len() / WORD_SIZE {
            self.done(status);
        } else {
            self.write_block(state);
        }
    }
}
