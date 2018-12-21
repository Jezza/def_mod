extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	#[cfg(windows)] = "sys/win.rs"
	#[cfg(not(windows))] = "sys/nix.rs"
	mod test {
		fn method(_: u32) -> u8;
		type Test {
			// Try changing the method's name/signature.
        	fn new() -> Test;
        }
	}

	mod other {
		fn method(_: u64, _: u8) -> u32;

		type MyStruct {
			fn new() -> Self;
			fn generic<T>(self, _: u32, other: T, func: fn(T) -> Self) -> Self;
		}
		fn generic<'a, T: 'a>(_: MyStruct, value: u32, other: &'a T, func: fn(T) -> MyStruct) -> MyStruct;
	}
}

fn main() {
}
