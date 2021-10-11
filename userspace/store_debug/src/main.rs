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

const FLASH_START: usize = 0x40000;
const STORAGE_START: usize = 0xBE000;
const WORD_SIZE: usize = 4;

fn test_index(index: usize, words: usize) {
    let mut console = Console::new();
    let mut storage = new_storage().unwrap();
    let page_size = storage.page_size();

    let offset = (STORAGE_START - FLASH_START) / WORD_SIZE;
    let page = (index - offset) / page_size;
    let byte = ((index - offset) % page_size) * WORD_SIZE;
    let addr = index * WORD_SIZE + FLASH_START;

    writeln!(
        console,
        "Testing index {:#X} (addr={:#X}, page={:#X}, byte={:#X}, words={})",
        index, addr, page, byte, words
    )
    .unwrap();
    console.flush();

    let result = storage
        .write_slice(StorageIndex { page, byte }, &vec![0xC3; words * 4])
        .unwrap();
    writeln!(console, " -- Write result {:?}\n", result).unwrap();
    console.flush();
}

fn test_address(addr: usize, words: usize) {
    test_index((addr - FLASH_START) / WORD_SIZE, words);
}

fn main() {
    let mut console = Console::new();
    let mut todo = Vec::new();

    writeln!(console, "\n *** Testing indices *** \n").unwrap();
    console.flush();

    todo.push((0x1F838, 1));
    todo.push((0x1F838, 2));
    todo.push((0x1F838, 4));
    todo.push((0x1F838, 8));
    todo.push((0x1F840, 1));
    todo.push((0x1F840, 2));
    todo.push((0x1F840, 4));
    todo.push((0x1F840, 8));
    todo = todo.into_iter().rev().collect();

    while let Some((index, length)) = todo.pop() {
        test_index(index, length);
    }

    writeln!(console, "\n *** Testing addresses *** \n").unwrap();
    console.flush();

    todo.push((0xBE000, 1));
    todo.push((0xBE000, 2));
    todo.push((0xBE000, 4));
    todo.push((0xBE000, 32));
    todo.push((0xBE100, 1));
    todo.push((0xBE100, 2));
    todo.push((0xBE100, 4));
    todo.push((0xBE100, 8));
    todo.push((0xBE100, 16));
    todo = todo.into_iter().rev().collect();

    while let Some((index, length)) = todo.pop() {
        test_address(index, length);
    }

    writeln!(console, "\n *** Triggering failure *** \n").unwrap();
    console.flush();
    test_address(0xBE0E0, 16);
}
