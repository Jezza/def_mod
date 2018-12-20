`def_mod!` provides a familiar syntax to the standard module declarations, but with the added benefit of being able
to easily define implementation routes and to statically verify module exports.

---
```rust
extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	// This has the exact same behaviour as Rust's.
	mod my_mod;

	// Much like the one above, it also has the same effect as Rust's.
	mod my_first_mod {
		// It also checks to see if a method with the type `fn(u32) -> u8` was exported.
    	// It will fail to compile if it finds none.
		fn method(_: u32) -> u8;

		// Much like the method declaration from above, this will check to see if a type was exported.
		type MyStruct;

		// This will also check to see if the type `MyOtherStruct` was export.
		type MyOtherStruct {
			// This will check if this method exists on this type. (MyOtherStruct::method)
			fn method(_: u32) -> u8;
		}
	}

	// Attributes are declared like normal.
	#[cfg(windows)]
	mod my_second_mod;

	// When declaring an attribute, you can optionally add a path.
	// This path is then used as the path attribute for the module file.
	// All attributes declared with a path are treated as _mutually exclusive_.
	// So a `mod` declaration is generated for each.
	// This makes it a lot easier to manage cross-platform code.
	// Note: attributes that don't have a path are copied to each module declaration.

	#[cfg(windows)] = "sys/win/mod.rs"
	#[cfg(not(windows))] = "sys/nix/mod.rs"
	mod sys;

	// Expands to:

	#[cfg(windows)]
	#[path = "sys/win/mod.rs"]
	mod sys;

	#[cfg(not(windows))]
	#[path = "sys/nix/mod.rs"]
	mod sys;

	// You can also declare attributes on methods or types themselves, and they will be used when verifying the type.
	// This module itself will be verified when not on a windows system.
	#[cfg(not(windows))]
	mod my_third_mod {
		// This method will only be verified when on linux.
		#[cfg(linux)]
		fn interop() -> u8;
		// Same with this type. It will only be verified when on a linux system
		#[cfg(linux)]
		type SomeStruct {
			fn interop() -> u8;
		}
	}
}

fn main() {
}
```

NOTE: that `def_mod` uses syntax tricks to assert for types.  
So that means when you use the path shorthand, it will still only check the module that is loaded, not all potential modules.    

Eg, if I've got a module that has two possible impls, one for windows and one for unix.  
If I compile on windows, `def_mod` can't check if the unix module has the correct symbols declared, because it's not the module that's compiled.  

You would need to explicitly enable compilation of the modules you'd want to check, because you can declare  
arbitrary #[cfg] attributes, there's no way in `def_mod` to do this.  
It's entirely up to you.  

---

In case you're curious as to what the macro generates:

A method assertion is transformed to something like:

```rust
const _VALUE: fn(u32) -> u8 = my_mod::method;
```

A type assertion is transformed into a use decl, inside their own scope:

```rust
{
	use self::my_mod::Test;
	// Any method assertions for the type will also be placed inside the same scope.
}
```

All of those are shoved into a function generated by the macro.

For example, given something like:

```rust
def_mod! {
	mod my_mod {
		fn plus_one(value: u8) -> u8;

		type MyStruct {
			fn new() -> MyStruct;
		}
	}
}
```

It'll turn it into something like this:

```rust
mod my_mod;

fn _load_my_mod() {
	const _ASSERT_METHOD_0: fn(u8) -> u8 = my_mod::method;

	{
		use self::my_mod::MyStruct;
		const _ASSERT_METHOD_1: fn() -> MyStruct = MyStruct::new;
	}
}
```

---

Disclaimer:

I'm still working on this, so it's possible not everything is implemented completely/correctly.  
I know the method type signature conversion still needs some working on.  
I want to add support for self, Self, and generics (Both lifetime and type).  

If you've got an idea or two, feel free to give me a poke.
