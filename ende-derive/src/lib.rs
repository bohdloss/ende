use proc_macro::TokenStream as TokenStream1;

use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, LifetimeParam, parse_quote, GenericParam};

use crate::ctxt::{Ctxt, Target};

mod ctxt;
mod enums;
mod flags;
mod generator;
mod parse;
mod lifetime;

static ENDE: &str = "ende";

/// Emulates the $crate available in regular macros
fn dollar_crate(name: &str) -> Ident {
    crate_name(name)
        .map(|x| match x {
            FoundCrate::Itself => Ident::new("crate", Span::call_site()),
            FoundCrate::Name(name) => Ident::new(&name, Span::call_site()),
        })
        .unwrap()
}

#[proc_macro_derive(Encode, attributes(ende))]
pub fn encode(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input as DeriveInput);
    let ctxt = match Ctxt::parse_from(&input, Target::Encode) {
        Ok(ctxt) => ctxt,
        Err(err) => return TokenStream1::from(err.to_compile_error()),
    };

    let (impl_generics, ty_generics, where_clause) = ctxt.generics.split_for_impl();
    let ref crate_name = ctxt.flags.crate_name;
    let ref item_name = ctxt.item_name;
    let ref encoder = ctxt.encoder;
    let ref encoder_generic = ctxt.encoder_generic;

    let body = match ctxt.derive() {
        Ok(ctxt) => ctxt,
        Err(err) => return TokenStream1::from(err.to_compile_error()),
    };

    quote!(
		#[automatically_derived]
		impl #impl_generics #crate_name::Encode for #item_name #ty_generics #where_clause {
			fn encode<#encoder_generic: #crate_name::io::Write>(&self, #encoder: &mut #crate_name::Encoder<#encoder_generic>) -> #crate_name::EncodingResult<()> {
				#body
			}
		}
	).into()
}

#[proc_macro_derive(Decode, attributes(ende))]
pub fn decode(input: TokenStream1) -> TokenStream1 {
    let input = parse_macro_input!(input as DeriveInput);
    let ctxt = match Ctxt::parse_from(&input, Target::Decode) {
        Ok(ctxt) => ctxt,
        Err(err) => return TokenStream1::from(err.to_compile_error()),
    };

    let (impl_generics, ty_generics, where_clause) = ctxt.generics.split_for_impl();
    let ref crate_name = ctxt.flags.crate_name;
    let ref item_name = ctxt.item_name;
    let ref encoder = ctxt.encoder;
    let ref encoder_generic = ctxt.encoder_generic;

    let body = match ctxt.derive() {
        Ok(ctxt) => ctxt,
        Err(err) => return TokenStream1::from(err.to_compile_error()),
    };

    quote!(
		#[automatically_derived]
		impl #impl_generics #crate_name::Decode for #item_name #ty_generics #where_clause {
			fn decode<#encoder_generic: #crate_name::io::Read>(#encoder: &mut #crate_name::Encoder<#encoder_generic>) -> #crate_name::EncodingResult<Self> {
				#body
			}
		}
	).into()
}

#[proc_macro_derive(BorrowDecode, attributes(ende))]
pub fn borrow_decode(input: TokenStream1) -> TokenStream1 {
	let input = parse_macro_input!(input as DeriveInput);
	let ctxt = match Ctxt::parse_from(&input, Target::BorrowDecode) {
		Ok(ctxt) => ctxt,
		Err(err) => return TokenStream1::from(err.to_compile_error()),
	};

	let body = match ctxt.derive() {
		Ok(ctxt) => ctxt,
		Err(err) => return TokenStream1::from(err.to_compile_error()),
	};
	
	let ref decoder_lif = ctxt.borrow_data.decoder;
	let lif: LifetimeParam = if ctxt.borrow_data.sub_lifetimes.is_empty() {
		parse_quote!(
			#decoder_lif
		)
	} else {
		let sub_lifs = ctxt.borrow_data.sub_lifetimes.iter();
		parse_quote!(
			#decoder_lif: #(#sub_lifs)+*
		)
	};

	// Inject the decoder's lifetime parameter in the `impl` generics
	let mut generics = ctxt.generics.clone();
	generics.params.insert(0, GenericParam::Lifetime(lif));
	
	// Impl generics use injected lifetime
	let (impl_generics, _, _) = generics.split_for_impl();
	// Ty and where clause use the original generics
	let (_, ty_generics, where_clause) = ctxt.generics.split_for_impl();
	let ref crate_name = ctxt.flags.crate_name;
	let ref item_name = ctxt.item_name;
	let ref encoder = ctxt.encoder;
	let ref encoder_generic = ctxt.encoder_generic;
	
	quote!(
		#[automatically_derived]
		impl #impl_generics #crate_name::BorrowDecode<#decoder_lif> for #item_name #ty_generics #where_clause {
			fn borrow_decode<#encoder_generic: #crate_name::io::BorrowRead<#decoder_lif>>(#encoder: &mut #crate_name::Encoder<#encoder_generic>) -> #crate_name::EncodingResult<Self> {
				#body
			}
		}
	).into()
}
