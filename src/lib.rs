#![feature(proc_macro_diagnostic)]

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream as TStream;

use proc_macro2::{TokenStream, Span};
use quote::{quote, quote_spanned, ToTokens};
use syn::*;
use syn::punctuated::Punctuated;
use syn::synom::{Parser, Synom};

#[proc_macro]
pub fn def_mod(tokens: TStream) -> TStream {
	let t = ModuleDecl::parse_all;
	let declarations: Vec<ModuleDecl> = t.parse(tokens).unwrap();

	let mut output = TokenStream::new();

	for module in declarations {
		let module_name = module.ident;

		let mut pathed_attrs = vec![];
		let mut custom_attrs = vec![];
		// Group the attributes by their "exclusivity" factor.
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

		if pathed_attrs.is_empty() {
			let vis = &module.vis;
			let t = quote_spanned! { module_name.span() =>
				#(#custom_attrs)*
				#vis mod #module_name;
			};
			t.to_tokens(&mut output);
		} else {
			for (attr, path) in pathed_attrs {
				let vis = &module.vis;
				let t = quote_spanned! { module_name.span() =>
					#attr
					#[path=#path]
					#(#custom_attrs)*
					#vis mod #module_name;
				};
				t.to_tokens(&mut output);
			}
		}

		// Generate a load function, if the module declared some items.
		if let ModuleBody::Content((_brace, body)) = module.body {
			let mut items: Vec<TokenStream> = vec![];

			let mut index: u32 = 0;
			for item in body {
				items.push(match item {
					DeclItem::Method(method_item) => {
						let i = gen_method_assertion(index, &module_name, method_item);
						index += 1;
						i
					},
					DeclItem::Type(type_item) => {
						let type_name = type_item.ident;
	
						let mut method_items = vec![];
						if let TypeDeclBody::Content((_brace, body)) = type_item.body {
							for method_item in body {
								method_items.push(gen_method_assertion(index, &type_name, method_item));
								index += 1;
							}
						}
	
						// We use the actual use declaration here to test for the type itself, as it'll fail if it doesn't exist or not exported.
						// It also makes the codegen easier, because we don't have to qualify the full name type.
						quote! {
							{
								use self::#module_name::#type_name;
								#(#method_items)*
							}
						}
					}
				});
			}
			let function_name = format!("__load_{}", module_name);
			let function_name = Ident::new(&function_name, Span::call_site());
			let t = quote! {
				#[allow(dead_code)]
				fn #function_name() {
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
/// The only difference between this and a normal mod is
/// the ability to add a path literal to attributes:
/// ```rust
/// #[cfg(target_os = "windows")] = "my_mod/win/mod.rs"
/// mod my_mod;
/// ```
/// 
#[derive(Debug)]
struct ModuleDecl {
	attrs: Vec<(Attribute, Option<LitStr>)>,
	vis: Visibility,
	mod_token: Token![mod],
	ident: Ident,
	body: ModuleBody,
}

#[derive(Debug)]
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

#[derive(Debug)]
enum DeclItem {
	Method(TraitItemMethod),
	Type(TypeDecl),
}

#[derive(Debug)]
struct TypeDecl {
	ident: Ident,
	body: TypeDeclBody,
}

#[derive(Debug)]
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

//Option<Vec<TraitItemMethod>>
impl Synom for TypeDecl {
	named!(parse -> Self, do_parse!(
			_type: keyword!(type) >>
			ident: syn!(Ident) >>
			body: alt!(
				punct!(;) => { TypeDeclBody::Terminated }
				|
				braces!(many0!(TraitItemMethod::parse)) => { TypeDeclBody::Content }
			) >>
			(TypeDecl {
				ident,
				body,
			})
		)
	);
}

fn gen_method_assertion(index: u32, context: &Ident, method_item: TraitItemMethod) -> TokenStream {
	let load_name = format!("_ASSERT_METHOD_{}", index);
	let load_ident = Ident::new(&load_name, Span::call_site());
	let (ty, path) = convert(context, method_item.sig);
	quote! {
		const #load_ident: #ty = #path;
	}
}

fn convert(context: &Ident, sig: MethodSig) -> (TypeBareFn, ExprPath) {
	// @TODO Jezza - 19 Dec. 2018: Generic path attributes?

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
		constness,
		unsafety,
		abi,
		ident,
		decl,
	} = sig;

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
				FnArg::SelfRef(_v) => {
					continue;
				}
				FnArg::SelfValue(_v) => {
					continue;
				}
				FnArg::Captured(ArgCaptured {
					pat,
					colon_token,
					ty,
				}) => {
					BareFnArg {
						name: None,
						ty
					}
				}
				FnArg::Inferred(_v) => {
					continue;
				}
				FnArg::Ignored(_v) => {
					continue;
				}
			};
			values.push(bare_fn_arg);
		}
		values
	};

	let type_bare_fn = TypeBareFn {
		unsafety,
		abi,
		fn_token,
		lifetimes: None,
		paren_token,
		inputs,
		variadic,
		output
	};

	let mut segments = Punctuated::new();
	segments.push(PathSegment {
		ident: (*context).clone(),
		arguments: PathArguments::None
	});
	segments.push(PathSegment {
		ident: ident.clone(),
		arguments: PathArguments::None
	});
	let path = Path {
		leading_colon: None,
		segments
	};
	let path = ExprPath {
		attrs: Vec::new(),
		qself: None,
		path
	};

//	TypeBareFn {
//		=pub unsafety: Option<Token![unsafe]>,
//		=pub abi: Option<Abi>,
//		=pub fn_token: Token![fn],
//		pub lifetimes: Option<BoundLifetimes>,
//		=pub paren_token: token::Paren,
//		pub inputs: Punctuated<BareFnArg, Token![,]>,
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

//	ExprPath {
//		pub attrs: Vec<Attribute>,
//		pub qself: Option<QSelf>,
//		pub path: Path,
//	}
//	pub struct Path {
//		pub leading_colon: Option<Token![::]>,
//		pub segments: Punctuated<PathSegment, Token![::]>,
//	}
//	pub struct PathSegment {
//		pub ident: Ident,
//		pub arguments: PathArguments,
//	}
	(type_bare_fn, path)
}
