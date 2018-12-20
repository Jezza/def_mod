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
//        	fn new0() -> Test;
        }
	}

	mod other {
		fn method(_: u64) -> u32;
	}
}

fn main() {
}
