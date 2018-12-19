#![feature(proc_macro_diagnostic)]

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream as TStream;
use std::fmt::Write;

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
		let module_name = module.ident.clone();

		if module.attrs.is_empty() {
			let vis = &module.vis;
			let t = quote_spanned! { module_name.span() =>
				#vis mod #module_name;
			};
			t.to_tokens(&mut output);
		} else {
			for attr in module.attrs {
				let meta = attr.interpret_meta()
					.expect("Invalid meta item :: must be of form #[os = \"path\"] :: not valid");
				let meta_name_value = match meta {
					Meta::NameValue(v) => v,
					_ => panic!("Invalid meta item :: mut be of form #[os = \"path\"] :: not a name value"),
				};
				let os = {
					let os_name = meta_name_value.ident;
					format!("{}", os_name)
				};

				let path = {
					let segment = match meta_name_value.lit {
						Lit::Str(v) => v.value(),
						_ => panic!("Invalid meta item :: mut be of form #[os = \"path\"] :: not a literal"),
					};

					if segment.starts_with("~") {
						let mut path = String::new();
						write!(path, "{}", module_name).unwrap();
						path.push('/');
						path.push_str(&segment[1..]);
						path.push_str("/mod.rs");
						path
					} else {
						segment
					}
				};

				let vis = &module.vis;
				let t = quote_spanned! { module_name.span() =>
					#[cfg(target_os = #os)]
					#[path=#path]
					#vis mod #module_name;
				};
				t.to_tokens(&mut output);
			}
		}

		let name = format!("__load_{}", module_name);
		let load_name = Ident::new(&name, Span::call_site());

		let mut index: u32 = 0;
		let mut tokenise_method_item = |context: &Ident, method_item: TraitItemMethod| {
			let load_name = format!("_ASSERT_METHOD_{}", index);
			index += 1;
			let load_ident = Ident::new(&load_name, Span::call_site());
			let (ty, path) = convert(context, method_item.sig);
			quote! {
				const #load_ident: #ty = #path;
			}
		};

		let mut items: Vec<TokenStream> = vec![];
		for item in module.body {
			items.push(match item {
				DeclItem::Method(method_item) => tokenise_method_item(&module_name, method_item),
				DeclItem::Type(type_item) => {
					let type_name = type_item.ident;

					let mut method_items = vec![];
					for method_item in type_item.body {
						method_items.push(tokenise_method_item(&type_name, method_item));
					}

					quote! {
						{
							use self::#module_name::#type_name;
							#(#method_items)*
						}
					}
				}
			});
		}

		let t = quote! {
			#[allow(dead_code)]
			fn #load_name() {
				#(#items)*
			}
		};
		t.to_tokens(&mut output);
	}
	output.into()
}

#[derive(Debug)]
struct ModuleDecl {
	attrs: Vec<Attribute>,
	vis: Visibility,
	mod_token: Token![mod],
	ident: Ident,
	body: Vec<DeclItem>,
}

impl ModuleDecl {
	named!(parse_all -> Vec<ModuleDecl>, do_parse!(
		decls: many0!(syn!(ModuleDecl)) >>
		(decls)
	));
}

impl Synom for ModuleDecl {
	named!(parse -> Self, do_parse!(
		attrs: many0!(Attribute::parse_outer) >>
		vis: syn!(Visibility) >>
		mod_token: keyword!(mod) >>
		ident: syn!(Ident) >>
		body: map!(braces!(many0!(DeclItem::parse)), |(_brace, vec)| vec) >>
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

impl DeclItem {
	named!(parse -> Self, alt!(
		syn!(TraitItemMethod) => { DeclItem::Method }
		|
		syn!(TypeDecl) => { DeclItem::Type }
	));
}

#[derive(Debug)]
struct TypeDecl {
	ident: Ident,
	body: Vec<TraitItemMethod>,
}

impl Synom for TypeDecl {
	named!(parse -> Self, do_parse!(
			_type: keyword!(type) >>
			ident: syn!(Ident) >>
			body: map!(braces!(many0!(TraitItemMethod::parse)), |(_brace, vec)| vec) >>
			(TypeDecl {
				ident,
				body,
			})
		)
	);
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
				FnArg::SelfRef(v) => {
					continue;
				}
				FnArg::SelfValue(v) => {
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
				FnArg::Inferred(v) => {
					continue;
				}
				FnArg::Ignored(v) => {
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
