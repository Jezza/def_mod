pub fn method(_: u64, _: u8) -> u32 {
	0
}

pub struct MyStruct;

impl MyStruct {
	pub fn new() -> Self {
		MyStruct
	}
	pub fn generic<T>(self, _: u32, _: T, _: fn(T) -> Self) -> Self {
		MyStruct
	}
}
pub fn generic<'a, T: 'a>(_: MyStruct, _: u32, _: &'a T, _: fn(T) -> MyStruct) -> MyStruct {
	MyStruct
}