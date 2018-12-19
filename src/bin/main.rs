extern crate def_mod;

use def_mod::def_mod;

fn main() {
}

def_mod! {
	#[linux = "test/nix/mod.rs"]
	#[macos = "~nix"]
	#[windows = "~windows"]
	mod test {
		fn method(_: u32) -> u8;
//		fn generic(_: u32) -> u8;
		type Test {
        	fn new() -> Test;
        }
	}
	mod other {
		fn method(_: u64) -> u32;
	}
}
