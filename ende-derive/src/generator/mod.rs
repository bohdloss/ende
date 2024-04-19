use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, TokenStreamExt, ToTokens};
use syn::{Expr, parse_quote, Type};

use crate::ctxt::{Ctxt, Field, ItemType, Scope, Target, Variant};
use crate::flags::{AllModifiers, Flags, Function, ModifierGroup, StreamModifier, TypeModifier};
use crate::generator::tokenize::CtxtToTokens;
use crate::parse::{Formatting};

pub mod encode;
pub mod decode;
mod tokenize;

/// As the name suggests, transforms an Option<&Expr> into an Expr that is either
/// Some(#expr) or None
fn option_expr_to_actual_option_expr(expr: Option<&Expr>) -> Expr {
	expr.as_ref()
		.map(|x| parse_quote!(Some(#x)))
		.unwrap_or(parse_quote!(None))
}

impl Ctxt {
	pub fn derive(&self) -> syn::Result<TokenStream2> {
		match self.target {
			Target::Encode => self.derive_encode(),
			Target::Decode => self.derive_decode(),
		}
	}
}

/// This is an aggregation of statements to allow access to fields inside `ende` flags expressions
#[derive(Clone)]
pub struct RefCode<'a> {
	ctxt: &'a Ctxt,
	code: TokenStream2,
}

impl<'a> RefCode<'a> {
	pub fn new(ctxt: &'a Ctxt) -> Self {
		Self {
			ctxt,
			code: TokenStream2::new(),
		}
	}

	pub fn append(&mut self, field: &Field) {
		match self.ctxt.target {
			Target::Encode => {
				// Only structs because enums already have references to their fields
				if let ItemType::Struct = self.ctxt.item_type {
					let ref name = field.name;
					let ref accessor = field.accessor;
					self.code.append_all(quote!(
					let ref #name = self.#accessor;
				));
				}
			}
			Target::Decode => {
				// Here the field already exists, but it is Owned, we need to create a Reference to it
				let ref name = field.name;
				self.code.append_all(quote!(
				let ref #name = #name;
			));
			}
		}
	}
}

impl ToTokens for RefCode<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		self.code.to_tokens(tokens);
	}
}

/// An aggregation of code that specifies the values of the different variants of an enum.
#[derive(Clone)]
pub struct ConstCode<'a> {
	ctxt: &'a Ctxt,
	code: TokenStream2,
}

impl<'a> ConstCode<'a> {
	pub fn new(ctxt: &'a Ctxt) -> Self {
		Self {
			ctxt,
			code: TokenStream2::new(),
		}
	}

	pub fn append(&mut self, variant: &Variant) {
		let enum_repr = self.ctxt.enum_repr;
		let ref ident = variant.index.ident;
		let ref expr = variant.index.expr;

		self.code.append_all(quote!(
			const #ident: #enum_repr = { #expr };
		));
	}
}

impl ToTokens for ConstCode<'_> {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		self.code.to_tokens(tokens);
	}
}

impl Function {
	pub fn derive_encode(&self, ctxt: &Ctxt, input: TokenStream2, ty: &Type) -> syn::Result<TokenStream2> {
		let ref crate_name = ctxt.flags.crate_name;
		let ref encoder = ctxt.encoder;
		Ok(match self {
			Function::Default => {
				quote!(<#ty as #crate_name::Encode>::encode(#input, #encoder)?)
			}
			Function::Serde(serde_crate) => {
				quote!(<#ty as #serde_crate::Serialize>::serialize(#input, &mut * #encoder)?)
			}
			Function::With(path, scope) => {
				match scope {
					Scope::Encode => quote!(#path(#input, &mut * #encoder)?),
					Scope::Decode => unreachable!(),
					Scope::Both => quote!(#path::encode(#input, &mut * #encoder)?),
				}
			}
			Function::Secret { encryption, public_key, private_key } => {
				let encryption = encryption
					.as_ref()
					.map(|x| x.ctxt_tokens(ctxt))
					.map(|x| syn::parse2::<Expr>(x).unwrap());
				let encryption = option_expr_to_actual_option_expr(encryption.as_ref());
				let public_key = option_expr_to_actual_option_expr(public_key.as_ref());
				let private_key = option_expr_to_actual_option_expr(private_key.as_ref());

				quote!(
					#crate_name::encryption::encode_asymm_block(
						#encoder,
						#encryption,
						#public_key,
						#private_key,
						#input
					)?
				)
			}
		})
	}

	pub fn derive_decode(&self, ctxt: &Ctxt, ty: &Type) -> syn::Result<TokenStream2> {
		let ref crate_name = ctxt.flags.crate_name;
		let ref encoder = ctxt.encoder;
		Ok(match self {
			Function::Default => {
				quote!(<#ty as #crate_name::Decode>::decode(#encoder)?)
			}
			Function::Serde(serde_crate) => {
				quote!(<#ty as #serde_crate::Deserialize>::deserialize(&mut * #encoder)?)
			}
			Function::With(path, scope) => {
				match scope {
					Scope::Encode => unreachable!(),
					Scope::Decode => quote!(#path(&mut * #encoder)?),
					Scope::Both => quote!(#path::decode(&mut * #encoder)?),
				}
			}
			Function::Secret { encryption, public_key, private_key } => {
				let encryption = encryption
					.as_ref()
					.map(|x| x.ctxt_tokens(ctxt))
					.map(|x| syn::parse2::<Expr>(x).unwrap());
				let encryption = option_expr_to_actual_option_expr(encryption.as_ref());
				let public_key = option_expr_to_actual_option_expr(public_key.as_ref());
				let private_key = option_expr_to_actual_option_expr(private_key.as_ref());

				quote!(
					#crate_name::encryption::decode_asymm_block(
						#encoder,
						#encryption,
						#public_key,
						#private_key,
					)?
				)
			}
		})
	}
}

impl TypeModifier {
	pub fn convert_into(&self, field: &Field) -> syn::Result<TokenStream2> {
		let ref name = field.name;
		let ref field_ty = field.ty;
		Ok(match self {
			TypeModifier::As(ty) => {
				parse_quote!(
					&(::core::clone::Clone::clone(#name) as #ty)
				)
			}
			TypeModifier::Convert(ty) => {
				parse_quote!(
					&(<#ty as ::core::convert::From<#field_ty>>::from(::core::clone::Clone::clone(#name)))
				)
			}
		})
	}

	pub fn convert_from(&self, field: &Field, input: TokenStream2) -> syn::Result<TokenStream2> {
		let ref field_ty = field.ty;
		Ok(match self {
			TypeModifier::As(_ty) => {
				parse_quote!(
					#input as #field_ty
				)
			}
			TypeModifier::Convert(ty) => {
				parse_quote!(
					<#field_ty as ::core::convert::From<#ty>>::from(#input)
				)
			}
		})
	}
}

impl ToTokens for Formatting {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		let ref format = self.format;
		let args: Vec<&Expr> = self.args
			.iter()
			.flat_map(|(_, x)| x.iter())
			.collect();

		tokens.append_all(quote!(
			#format, #(#args),*
		))
	}
}

impl Flags {
	/// Derives validation code for a field about to be encoded or which has just been decoded
	pub fn derive_validation(&self, ctxt: &Ctxt, ref_code: &RefCode) -> syn::Result<TokenStream2> {
		Ok(if let Some((validate, fmt)) = &self.validate {
			let ref crate_name = ctxt.flags.crate_name;

			let format = if let Some(fmt) = fmt.as_ref() {
				quote!(::core::format_args!(#fmt))
			} else {
				quote!(::core::format_args!("Assertion failed"))
			};

			quote!(
				if !{
					#ref_code
					#validate
				} {
					return Err(#crate_name::EncodingError::validation_error(#format));
				}
			)
		} else { quote!() })
	}
}

impl ModifierGroup {
	pub fn derive(&self, ctxt: &Ctxt) -> syn::Result<(Vec<TokenStream2>, Vec<TokenStream2>, Vec<TokenStream2>)> {
		let ref encoder = ctxt.encoder;
		let ref target = self.target;

		let mut save: Vec<TokenStream2> = Vec::new();
		let mut set: Vec<TokenStream2> = Vec::new();
		let mut restore: Vec<TokenStream2> = Vec::new();

		if let Some(num_encoding) = self.num_encoding {
			let num_encoding = num_encoding.ctxt_tokens(ctxt);
			let save_state = format_ident!("__{}_num_encoding", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.num_encoding;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.num_encoding = #num_encoding;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.num_encoding = #save_state;
			));
		}

		if let Some(endianness) = self.endianness {
			let endianness = endianness.ctxt_tokens(ctxt);
			let save_state = format_ident!("__{}_endianness", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.endianness;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.endianness = #endianness;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.endianness = #save_state;
			));
		}

		if let Some(bit_width) = self.bit_width {
			let bit_width = bit_width.ctxt_tokens(ctxt);
			let save_state = format_ident!("__{}_bit_width", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.width;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.width = #bit_width;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.width = #save_state;
			));
		}

		if let Some(max_size) = &self.max {
			let save_state = format_ident!("__{}_max_size", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.max_size;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.max_size = #max_size;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.max_size = #save_state;
			));
		}

		if let Some(str_encoding) = self.str_encoding {
			let str_encoding = str_encoding.ctxt_tokens(ctxt);
			let save_state = format_ident!("__{}_str_encoding", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.str_encoding;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.str_encoding = #str_encoding;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.str_encoding = #save_state;
			));
		}

		if let Some(str_len_encoding) = self.str_len_encoding {
			let str_len_encoding = str_len_encoding.ctxt_tokens(ctxt);
			let save_state = format_ident!("__{}_str_len_encoding", target.to_string());
			save.push(quote!(
				let #save_state = #encoder.ctxt.settings.#target.len_encoding;
			));
			set.push(quote!(
				#encoder.ctxt.settings.#target.len_encoding = #str_len_encoding;
			));
			restore.push(quote!(
				#encoder.ctxt.settings.#target.len_encoding = #save_state;
			));
		}

		Ok((save, set, restore))
	}
}

impl AllModifiers {
	pub fn derive(&self, ctxt: &Ctxt) -> syn::Result<(TokenStream2, TokenStream2)> {
		let ref encoder = ctxt.encoder;

		let mut save: Vec<TokenStream2> = Vec::new();
		let mut set: Vec<TokenStream2> = Vec::new();
		let mut restore: Vec<TokenStream2> = Vec::new();

		let (num_save, num_set, num_restore) = self.num.derive(ctxt)?;
		let (size_save, size_set, size_restore) = self.size.derive(ctxt)?;
		let (variant_save, variant_set, variant_restore) = self.variant.derive(ctxt)?;
		let (string_save, string_set, string_restore) = self.string.derive(ctxt)?;
		
		save.extend(num_save);
		save.extend(size_save);
		save.extend(variant_save);
		save.extend(string_save);

		set.extend(num_set);
		set.extend(size_set);
		set.extend(variant_set);
		set.extend(string_set);

		restore.extend(num_restore);
		restore.extend(size_restore);
		restore.extend(variant_restore);
		restore.extend(string_restore);

		if let Some(flatten) = &self.flatten {
			save.push(quote!(
				let __flatten = #encoder.ctxt.flatten;
			));
			set.push(quote!(
				#encoder.ctxt.flatten = Some(#flatten);
			));
			restore.push(quote!(
				#encoder.ctxt.flatten = __flatten;
			));
		}

		Ok((quote!(
			#(#save)*
			#(#set)*
		), quote!(
			#(#restore)*
		)))
	}
}

impl Flags {
	pub fn derive_stream_modifiers(&self, ctxt: &Ctxt, mut input: TokenStream2) -> syn::Result<TokenStream2> {
		for stream_modifier in self.stream_modifiers.iter() {
			input = stream_modifier.derive(ctxt, input)?;
		}
		Ok(input)
	}
}

impl StreamModifier {
	pub fn derive(&self, ctxt: &Ctxt, input: TokenStream2) -> syn::Result<TokenStream2> {
		let ref crate_name = ctxt.flags.crate_name;
		let ref encoder = ctxt.encoder;

		Ok(match self {
			StreamModifier::Encrypted { encryption, key, iv } => {
				let encryption = encryption
					.as_ref()
					.map(|x| x.ctxt_tokens(ctxt))
					.map(|x| syn::parse2::<Expr>(x).unwrap());
				let encryption = option_expr_to_actual_option_expr(encryption.as_ref());
				let key = option_expr_to_actual_option_expr(key.as_ref());
				let iv = option_expr_to_actual_option_expr(iv.as_ref());

				match ctxt.target {
					Target::Encode => {
						quote!(
							#crate_name::encryption::encode_with_encryption(
								#encoder,
								#encryption,
								#key,
								#iv,
								|#encoder| {
									#input
									Ok(())
								},
							)?;
						)
					}
					Target::Decode => {
						quote!(
							#crate_name::encryption::decode_with_encryption(
								#encoder,
								#encryption,
								#key,
								#iv,
								|#encoder| { Ok({ #input }) },
							)?
						)
					}
				}
			}
			StreamModifier::Compressed { compression } => {
				let compression = compression
					.as_ref()
					.map(|x| x.ctxt_tokens(ctxt))
					.map(|x| syn::parse2::<Expr>(x).unwrap());
				let compression = option_expr_to_actual_option_expr(compression.as_ref());

				match ctxt.target {
					Target::Encode => {
						quote!(
							#crate_name::compression::encode_with_compression(
								#encoder,
								#compression,
								|#encoder| {
									#input
									Ok(())
								},
							)?;
						)
					}
					Target::Decode => {
						quote!(
							#crate_name::compression::decode_with_compression(
								#encoder,
								#compression,
								|#encoder| { Ok({ #input }) },
							)?
						)
					}
				}
			}
		})
	}
}