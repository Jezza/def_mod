pub fn method(value: u32) -> u8 {
	0
}

pub fn generic<T>() -> T {
	unimplemented!()
}

pub struct Test;

impl Test {
	pub fn new() -> Test {
		Test
	}
}