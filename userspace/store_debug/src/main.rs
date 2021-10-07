// Copyright 2019-2020 Google LLC
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

#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write;
use embedded_flash::new_storage;
use libtock_drivers::console::Console;
use persistent_store::{Storage, StorageIndex};

libtock_core::stack_size! {0x2000}

fn main() {
    let mut console = Console::new();

    let mut storage = new_storage().unwrap();
    let num_pages = storage.num_pages();
    let page_size = storage.page_size();
    const MAX_LENGTH: usize = 512;
    let pattern: u8 = 0x00;
    for page in 0..num_pages {
        write!(console, "Erase page {}:", page).unwrap();
        console.flush();
        let result = storage.erase_page(page);
        writeln!(console, "{:?}", result).unwrap();
        console.flush();
        let mut todo = Vec::new();
        for i in 0..page_size / MAX_LENGTH {
            todo.push((i * MAX_LENGTH, MAX_LENGTH));
        }
        while let Some((byte, mut length)) = todo.pop() {
            let index = StorageIndex { page, byte };
            write!(console, "Write byte {:#x} length {:#x}:", byte, length).unwrap();
            console.flush();
            let result = storage.write_slice(index, &vec![pattern; length]);
            writeln!(console, "{:?}", result).unwrap();
            console.flush();
            if result.is_err() && length > 1 {
                let slice = storage.read_slice(index, length).unwrap();
                let ok_length = slice.iter().take_while(|&&x| x == pattern).count();
                writeln!(console, "Read length {:#x}", ok_length).unwrap();
                console.flush();
                length /= 2;
                if ok_length < length {
                    todo.push((byte + length, length));
                    todo.push((byte + ok_length, length - ok_length));
                } else {
                    todo.push((byte + ok_length, 2 * length - ok_length));
                }
            }
        }
    }
}
