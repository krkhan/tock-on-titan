// Copyright 2019 Google LLC
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

/// Works the fake through a series of writes and erases to test its
/// functionality. Simulates both failed operations and successful operations.
#[test]
fn fake_hw() -> bool {
	use { h1b::hil::flash::Hardware, test::require };
	let fake = h1b::hil::flash::fake::FakeHw::new();

	// Verify the initial state of the flash.
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFFFFFF);
	require!(fake.read(1301) == 0xFFFFFFFF);

	// Operation 1: successful write to two words.
	fake.set_transaction(1300, 2 - 1);
	fake.set_write_data(&[0xFFFF0FFF, 0xFFFAFFFF]);
	fake.trigger(h1b::hil::flash::driver::WRITE_OPCODE);
	require!(fake.is_programming() == true);
	fake.finish_operation();
	require!(fake.read_error() == 0);
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFF0FFF);
	require!(fake.read(1301) == 0xFFFAFFFF);

	// Operation 2: failed write. Verifies the write doesn't change anything.
	fake.set_transaction(1300, 2 - 1);
	fake.set_write_data(&[0xFFFF00FF, 0xFFAAFFFF]);
	fake.trigger(h1b::hil::flash::driver::WRITE_OPCODE);
	require!(fake.is_programming() == true);
	fake.inject_error(0x8);  // Program failed
	require!(fake.read_error() == 0x8);
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFF0FFF);
	require!(fake.read(1301) == 0xFFFAFFFF);

	// Operation 3: successful write to one word. Verifies the write doesn't
	// overlap to the next word.
	fake.set_transaction(1300, 1 - 1);
	fake.set_write_data(&[0xFFFFC0FF]);
	fake.trigger(h1b::hil::flash::driver::WRITE_OPCODE);
	require!(fake.is_programming() == true);
	fake.finish_operation();
	require!(fake.read_error() == 0);
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFF00FF);
	require!(fake.read(1301) == 0xFFFAFFFF);

	// Operation 4: successful erase of the second page. Confirms the erase
	// does not affect the third page.
	fake.set_transaction(512, 0);
	require!(fake.is_programming() == false);
	fake.trigger(h1b::hil::flash::driver::ERASE_OPCODE);
	require!(fake.is_programming() == true);
	fake.finish_operation();
	require!(fake.read_error() == 0);
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFF00FF);
	require!(fake.read(1301) == 0xFFFAFFFF);

	// Operation 5: successful erase of the third page. Verifies the erase
	// affects the values in the third page.
	fake.set_transaction(1024, 0);
	require!(fake.is_programming() == false);
	fake.trigger(h1b::hil::flash::driver::ERASE_OPCODE);
	require!(fake.is_programming() == true);
	fake.finish_operation();
	require!(fake.read_error() == 0);
	require!(fake.is_programming() == false);
	require!(fake.read(512) == 0xFFFFFFFF);
	require!(fake.read(1023) == 0xFFFFFFFF);
	require!(fake.read(1300) == 0xFFFFFFFF);
	require!(fake.read(1301) == 0xFFFFFFFF);

	true
}
