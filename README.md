`def_mod!` is a proc macro that enforces module exports nicely.
It also offers a nice way of declaring operating system specific modules. (Useful for cross-platform code.)

For example:
```rust
extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	mod my_mod {
		// If my_mod doesn't declare a public zero method, this will fail to compile.
		fn zero() -> u8;
	}
}

fn main() {
	let value = my_mod::zero();
}
```

Yes, as you may have noticed, this doesn't have any benefit.
And you'd be partially right, but there's two more things to consider.

First:

As the project gets bigger and bigger it gets hard to track what methods and types a module needs to export.
This means that all of the methods you require to be exported would be listed here.
Of course, this isn't a silver bullet.
This only checks if the items you've written exist, not if that's all of them, etc.

Second:
When you're writing cross-platform code, you tend to want to pack the actual os specific part away in
their own modules, not mucking up the general codebase.

This is where this starts making a bit more sense.
Not much more, as I'll explain at the end, but some, because you can also declare path attributes on modules,
and this macro just makes it easier.

```rust
extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	// If you start it with a tilde '~', then it'll automatically expand the path to:
	// "${module_name}/${value}/mod.rs"
	// For example, the windows attr will expand to:
	// "my_mod/win/mod.rs"
	// That's where Rust will look for the module file on windows.
	// If you omit the tilde, it will be used as the path attribute. (See the linux attr)
	#[windows = "~win"]
	#[linux = "my_mod/linux.rs"]
	mod my_mod {
		// Now if my_mod doesn't declare a public zero method, this will fail to compile.
		fn zero() -> u8;
	}
}

fn main() {
	let value = my_mod::zero();
}
```

However, this isn't perfect.
Due to the current implementation, it doesn't check ALL variants.
So, in other words, you won't know if a module file for another operating system contains a method unless you try
compiling it on that machine.

So, in other words, this whole macro thing doesn't really do anything special...
In fact, arguably, it gives you nothing over what you could probably do manually.

The main reason why this exists was because I had the idea, and as I worked on it, dreams started to be crushed.

Ideally, I'd want full os support, and you'll be able to tell if a os-specific impl of a module contains all of the
necessary items, but I think I'll need a compiler plugin for that, as I don't think sneaky syntax will help...

So, for what it's worth, enjoy this crate.



Oh, completely forgot to mention, you can also check for types and methods on those types:

```rust
extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	mod my_mod {
		type Zero {
			fn zero() -> u8;
		}
		type One {
			fn one() -> u8;
		}
	}
}
```
