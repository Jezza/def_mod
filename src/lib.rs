/*!
`def_mod!` provides a familiar syntax to the standard module declarations, but with the added benefit
of simpler implementation routing and statically verified module exports.

---
```rust
extern crate def_mod;

use def_mod::def_mod;

def_mod! {
	// This has the exact same behaviour as Rust's.
	mod my_mod;

	// Much like the one above, it also has the same effect as Rust's.
	mod my_first_mod {
		// This will check if a method with the name `method` and type `fn(u32) -> u8` was exported.
    	// It will fail to compile if it finds none.
		fn method(_: u32) -> u8;

		// Much like the method declaration from above, this will check to see if a type was exported.
		type MyStruct;

		// Much like the normal method check, that functionality is also extended to types.
		// So you can check if a type has a specific method exported.
		type MyOtherStruct {
			// This will check if this method exists on this type. (MyOtherStruct::method)
			fn method(_: u32) -> u8;
		}
	}

	// You can declare attributes like normal.
	#[cfg(windows)]
	mod my_second_mod;

	// When declaring an attribute, you can optionally add a string literal.
	// This literal is used as the path attribute for the module file.
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
		// Same with this type. It will only be verified when on a macos system
		#[cfg(macos)]
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

One good example is when you have two different modules for specific platforms.  
The module that would be compiled normally is the only one that would be checked.  
You would need to explicitly enable compilation of the modules if you want them to be checked too.  
Because you can declare arbitrary #[cfg] attributes, there's no generic way for `def_mod` to know how to do this.    
It's entirely up to you.  

---

In case you're curious as to what the macro generates:

A method assertion is transformed to something like:

```rust
fn method(_: u32) -> u8;

// into

const _VALUE: fn(u32) -> u8 = my_mod::method;
```

A method assertion with generics is a bit more complex:

```rust
fn generic<'a , T: 'a>(_: u32, _: T, _: fn(T) -> T) -> &'a T;

// will turn into into (Note: This is nested inside of the load function itself.)

#[allow(non_snake_case)]
fn _load_module_name_generic<'a, T: 'a>() {
	let _VALUE:
			fn(_: u32, _: T,
			   _: fn(T) -> T) -> &'a T =
		other::generic;
}
```

A type assertion is transformed into a new scope with a use declaration:

```rust
def_mod! {
	mod my_mod {
		type Test;
	}
}

// Into

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
			fn new() -> Self;
			fn dupe(&self) -> Self;
			fn clear(&mut self);
		}

		fn generic<'a , T: 'a>(_: u32, _: T, _: fn(T) -> T) -> &'a T;
	}
}
```

It'll turn it into something like this:

```rust
mod my_mod;

fn _load_my_mod() {
	const _ASSERT_METHOD_0: fn(u8) -> u8 = self::my_mod::plus_one;
	{
		use self::my_mod::MyStruct;
		const _ASSERT_METHOD_1: fn() -> MyStruct = MyStruct::new;
		const _ASSERT_METHOD_2: fn(_self: &MyStruct) -> MyStruct = MyStruct::dupe;
		const _ASSERT_METHOD_3: fn(_self: &mut MyStruct) -> MyStruct = MyStruct::clear;
	}
	#[allow(non_snake_case)]
    fn _load_my_mod_generic<'a, T: 'a>() {
    	let _ASSERT_METHOD_4:
    			fn(_: u32, _: T,
    			   _: fn(T) -> T) -> &'a T =
    		my_mod::generic;
    }
}
```
*/

#![feature(proc_macro_diagnostic)]

#[macro_use]
extern crate proc_macro;

#[macro_use]
extern crate proc_macro2;

#[macro_use]
extern crate quote;

#[macro_use]
extern crate syn;

use proc_macro::TokenStream as TStream;

use proc_macro2::{TokenStream, TokenTree, Group};
use quote::{quote, quote_spanned, ToTokens, TokenStreamExt};
use syn::*;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::synom::{Parser, Synom};

#[proc_macro]
pub fn def_mod(tokens: TStream) -> TStream {
	let t = ModuleDecl::parse_all;
	let declarations: Vec<ModuleDecl> = t.parse(tokens).unwrap();

	let mut output = TokenStream::new();

	for module in declarations {
		let mut pathed_attrs = vec![];
		let mut custom_attrs = vec![];
		// Group the attributes that were declared with a path value.
		for attr in module.attrs {
			if let (attr, Some(path)) = attr {
				pathed_attrs.push((attr, path));
			} else {
				custom_attrs.push(attr.0);
			};
		}
		// Ghost the attr vectors, so no one can change them...
		let pathed_attrs = &pathed_attrs;
		let custom_attrs = &custom_attrs;

		let vis = &module.vis;
		let mod_token = &module.mod_token;
		let module_name = &module.ident;

		if pathed_attrs.is_empty() {
			let t = quote_spanned! { module_name.span() =>
				#(#custom_attrs)*
				#vis #mod_token #module_name;
			};
			t.to_tokens(&mut output);
		} else {
			// If there were some pathed attrs, then we need to generate an equal number of mod decls with the attrs.
			for (attr, path) in pathed_attrs {
				let t = quote_spanned! { module_name.span() =>
					#attr
					#[path=#path]
					#(#custom_attrs)*
					#vis #mod_token #module_name;
				};
				t.to_tokens(&mut output);
			}
		}

		// Generate a load function, if the module was declared with some items.
		if let ModuleBody::Content((_brace, body)) = module.body {
			let mut index: u32 = 0;
			// This is the function that transforms a method into an assertion.
			let mut tokenise_method = |type_name: Option<&Ident>, method_item: TraitItemMethod| {
				if let Some(body) = method_item.default {
					body.span()
						.unstable()
						.error("A body isn't valid here.")
						.emit();
					return TokenStream::new();
				}
				let mapping = if let Some(type_name) = type_name {
					let self_replacement = type_name.to_string();
					let func = move |ident: Ident| {
						// @FIXME Jezza - 21 Dec. 2018: Yeah, this is very... eh... yucky...
						// I can't think of a better way to do this...
						if ident.to_string() == "Self" {
							Ident::new(&self_replacement, ident.span())
						} else {
							ident
						}
					};
					Some(func)
				} else {
					None
				};
				let t = convert(module_name, type_name, index, method_item, mapping);
				index += 1;
				t
			};
			let items: Vec<TokenStream> = body.into_iter()
				.map(|item| {
					// Transform each item into the corresponding check.
					match item {
						DeclItem::Method(method_item) => tokenise_method(None, method_item),
						DeclItem::Type(type_item) => {
							let attrs = &type_item.attrs;
							let type_name = &type_item.ident;

							let method_items = if let TypeDeclBody::Content((_brace, body)) = type_item.body {
								body.into_iter()
									.map(|method_item| tokenise_method(Some(type_name), method_item))
									.collect()
							} else {
								vec![]
							};

							// We use the actual use declaration here to test for the type itself, as it'll fail if it doesn't exist or not exported.
							// It also makes the codegen easier, because we don't have to qualify the full name type.
							quote! {
								#(#attrs)*
								{
									use self::#module_name::#type_name;
									#(#method_items)*
								}
							}
						}
					}
				})
				.filter(|t| !t.is_empty())
				.collect();

			let function_name = {
				let name = format!("_load_{}", module_name);
				Ident::new(&name, module_name.span())
			};
			let t = quote! {
				#[allow(dead_code)]
				fn #function_name() {
					use self::#module_name::*;
					#(#items)*
				}
			};
			t.to_tokens(&mut output);
		}
	}
	output.into()
}

///
/// A module declaration: `mod my_mod`
/// 
/// One of the differences between this and a normal mod decl is
/// the attributes can declare a path literal:
/// ```rust
/// #[cfg(target_os = "windows")] = "my_mod/win/mod.rs"
/// mod my_mod;
/// ```
/// 
/// The other is that it can declare a body.
/// The body contains methods/types that the module needs to export.
/// If the module doesn't export those symbols, you will get a compiler error.
/// 
#[cfg_attr(feature = "derive-debug", derive(Debug))]
struct ModuleDecl {
	attrs: Vec<(Attribute, Option<LitStr>)>,
	vis: Visibility,
	mod_token: Token![mod],
	ident: Ident,
	body: ModuleBody,
}

#[cfg_attr(feature = "derive-debug", derive(Debug))]
enum ModuleBody {
	Content((token::Brace, Vec<DeclItem>)),
	Terminated(Token![;]),
}

impl ModuleDecl {
	named!(parse_all -> Vec<ModuleDecl>, do_parse!(
		decls: many0!(syn!(ModuleDecl)) >>
		(decls)
	));
}

impl Synom for ModuleDecl {
	named!(parse -> Self, do_parse!(
		attrs: many0!(do_parse!(
			attr: call!(Attribute::parse_outer) >>
			eq: option!(punct!(=)) >>
			path: cond!(eq.is_some(), syn!(LitStr))>>
			(attr, path)
		)) >>
		vis: syn!(Visibility) >>
		mod_token: keyword!(mod) >>
		ident: syn!(Ident) >>
		body: alt! (
			punct!(;) => { ModuleBody::Terminated }
			|
			braces!(many0!(DeclItem::parse)) => { ModuleBody::Content }
		) >>
		(ModuleDecl {
			attrs,
			vis,
			mod_token,
			ident,
			body,
		})
	));
}

impl ToTokens for ModuleDecl {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		self.vis.to_tokens(tokens);
	}
}

#[cfg_attr(feature = "derive-debug", derive(Debug))]
enum DeclItem {
	Method(TraitItemMethod),
	Type(TypeDecl),
}

#[cfg_attr(feature = "derive-debug", derive(Debug))]
struct TypeDecl {
	attrs: Vec<Attribute>,
	ident: Ident,
	body: TypeDeclBody,
}

#[cfg_attr(feature = "derive-debug", derive(Debug))]
enum TypeDeclBody {
	Content((token::Brace, Vec<TraitItemMethod>)),
	Terminated(Token![;]),
}

impl DeclItem {
	named!(parse -> Self, alt!(
		syn!(TraitItemMethod) => { DeclItem::Method }
		|
		syn!(TypeDecl) => { DeclItem::Type }
	));
}

impl Synom for TypeDecl {
	named!(parse -> Self, do_parse!(
			attrs: many0!(Attribute::parse_outer) >>
			_type: keyword!(type) >>
			ident: syn!(Ident) >>
			body: alt!(
				punct!(;) => { TypeDeclBody::Terminated }
				|
				braces!(many0!(TraitItemMethod::parse)) => { TypeDeclBody::Content }
			) >>
			(TypeDecl {
				attrs,
				ident,
				body,
			})
		)
	);
}

fn convert<F>(module_name: &Ident, type_name: Option<&Ident>, index: u32, method_item: TraitItemMethod, ident_mapping: Option<F>) -> TokenStream
		where F: Fn(Ident) -> Ident {

//	println!("Context: {}", context);
//	println!("Sig: {:?}", sig);
//	pub struct MethodSig {
//		pub constness: Option<Token![const]>,
//		pub unsafety: Option<Token![unsafe]>,
//		pub abi: Option<Abi>,
//		pub ident: Ident,
//		pub decl: FnDecl,
//	}
//	pub struct FnDecl {
//		pub fn_token: Token![fn],
//		pub generics: Generics,
//		pub paren_token: token::Paren,
//		pub inputs: Punctuated<FnArg, Token![,]>,
//		pub variadic: Option<Token![...]>,
//		pub output: ReturnType,
//	}
//	pub enum FnArg {
//		/// Self captured by reference in a function signature: `&self` or `&mut
//		/// self`.
//		///
//		/// *This type is available if Syn is built with the `"full"` feature.*
//		pub SelfRef(ArgSelfRef {
//			pub and_token: Token![&],
//			pub lifetime: Option<Lifetime>,
//			pub mutability: Option<Token![mut]>,
//			pub self_token: Token![self],
//		}),
//	
//		/// Self captured by value in a function signature: `self` or `mut
//		/// self`.
//		///
//		/// *This type is available if Syn is built with the `"full"` feature.*
//		pub SelfValue(ArgSelf {
//			pub mutability: Option<Token![mut]>,
//			pub self_token: Token![self],
//		}),
//	
//		/// An explicitly typed pattern captured by a function signature.
//		///
//		/// *This type is available if Syn is built with the `"full"` feature.*
//		pub Captured(ArgCaptured {
//			pub pat: Pat,
//			pub colon_token: Token![:],
//			pub ty: Type,
//		}),
//	
//		/// A pattern whose type is inferred captured by a function signature.
//		pub Inferred(Pat),
//		/// A type not bound to any pattern in a function signature.
//		pub Ignored(Type),
//	}
//	pub struct Generics {
//		pub lt_token: Option<Token![<]>,
//		pub params: Punctuated<GenericParam, Token![,]>,
//		pub gt_token: Option<Token![>]>,
//		pub where_clause: Option<WhereClause>,
//	}
	let MethodSig {
		constness: _,
		unsafety,
		abi,
		ident,
		decl,
	} = method_item.sig;

	let FnDecl {
		fn_token,
		generics,
		paren_token,
		inputs,
		variadic,
		output,
	} = decl;

	let inputs = {
		let mut values = Punctuated::new();
		for arg in inputs {
			let bare_fn_arg = match arg {
				FnArg::SelfRef(ArgSelfRef{
					and_token,
					lifetime,
					mutability,
					self_token: _
				}) => {
					let t = quote! {
						_self: #and_token #lifetime #mutability #type_name
					};
					parse2::<BareFnArg>(t).expect("Should never happen [self-ref]")
				}
				FnArg::SelfValue(ArgSelf {
					mutability,
					self_token: _,
				}) => {
					let t = quote! {
						_self: #mutability #type_name
					};
					parse2::<BareFnArg>(t).expect("Should never happen [self-value]")
				}
				FnArg::Captured(ArgCaptured {
					pat,
					colon_token,
					ty,
				}) => {
					let ts = ty.into_token_stream();
					let ty = if let Some(ref func) = ident_mapping {
						replace_idents(ts, func)
					} else {
						ts
					};
					let t: TokenStream = quote! {
						#pat #colon_token #ty
					};
					parse2::<BareFnArg>(t).expect("Should never happen [arg-captured]")
				}
				FnArg::Inferred(pat) => {
					// Technically, this shouldn't be possible, but we know the transformation that needs to take place,
					// so I just go ahead and do that.
					let t = quote! {
						#pat: _
					};
					parse2::<BareFnArg>(t).expect("Should never happen [inferred]")
				}
				FnArg::Ignored(ty) => {
					// This way of writing signatures has been deprecated, and I should probably emit a warning.
					ident.span()
						.unstable()
						.warning("Declaring parameters without a name is deprecated, and will not be supported in the future. [Hint: Just add \"_:\" to the parameter to remove this warning...]")
						.emit();
					let ts = ty.into_token_stream();
					let ts = if let Some(ref func) = ident_mapping {
						replace_idents(ts, func)
					} else {
						ts
					};
					parse2::<BareFnArg>(ts).expect("Should never happen [ignored]")
				}
			};
			values.push(bare_fn_arg);
		}
		values
	};

	let output = {
		let ts = output.into_token_stream();
		let ts = if let Some(ref func) = ident_mapping {
			replace_idents(ts, func)
		} else {
			ts
		};
		parse2::<ReturnType>(ts).expect("Should never happen [return-type]")
	};

	let type_bare_fn = TypeBareFn {
		unsafety,
		abi,
		fn_token,
		lifetimes: None,
		paren_token,
		inputs,
		variadic,
		output,
	};

//	TypeBareFn {
//		=pub unsafety: Option<Token![unsafe]>,
//		=pub abi: Option<Abi>,
//		=pub fn_token: Token![fn],
//		~pub lifetimes: Option<BoundLifetimes>,
//		=pub paren_token: token::Paren,
//		~pub inputs: Punctuated<BareFnArg, Token![,]>,
//		=pub variadic: Option<Token![...]>,
//		=pub output: ReturnType,
//	}
//	pub struct BareFnArg {
//		pub name: Option<(BareFnArgName, Token![:])>,
//		pub ty: Type,
//	}
//	pub enum BareFnArgName {
//		/// Argument given a name.
//		Named(Ident),
//		/// Argument not given a name, matched with `_`.
//		Wild(Token![_]),
//	}
	let attrs = &method_item.attrs;
	let load_ident = {
		let name = format!("_ASSERT_METHOD_{}", index);
		Ident::new(&name, ident.span())
	};
	let context = type_name.unwrap_or(module_name);

	if generics.params.is_empty() {
		quote! {
			#(#attrs)*
			const #load_ident: #type_bare_fn = #context::#ident;
		}
	} else {
		let nested_function_name = {
			let name = if let Some(type_name) = type_name {
				format!("_load_{}_{}_{}", module_name, type_name, ident)
			} else {
				format!("_load_{}_{}", module_name, ident)
			};
			Ident::new(&name, ident.span())
		};
		// Do note that we don't use ty_generics, as it's just the use-site, which for us is in the method's signature.
		let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
		quote! {
			#(#attrs)*
			#[allow(non_snake_case)]
			fn #nested_function_name #impl_generics() #where_clause {
				let #load_ident: #type_bare_fn = #context::#ident;
			}
		}
	}
}

fn replace_idents<F>(ts: TokenStream, func: &F) -> TokenStream
		where F: Fn(Ident) -> Ident {
	let mut out = TokenStream::new();
	ts.into_iter()
		.map(move |tt| {
			match tt {
				TokenTree::Group(g) => {
					let delimiter = g.delimiter();
					let ts = g.stream();
					let out = replace_idents(ts, func);
					TokenTree::Group(Group::new(delimiter, out))
				},
				TokenTree::Ident(i) => TokenTree::Ident(func(i)),
				v => v,
			}
		})
		.for_each(|tt| out.append(tt));
	out
}