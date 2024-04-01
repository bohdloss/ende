use proc_macro2::Ident;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use syn::{Error, Expr, parse_quote, Path, Type};
use syn::spanned::Spanned;

use crate::{dollar_crate, ENDE};
use crate::ctxt::Scope;
use crate::enums::{BitWidth, Endianness, NumEncoding};
use crate::parse::{AsConversion, CompressionConstructor, EncryptionConstructor, EncryptionData, Flag, Formatting, Modifier, ModTarget, SecretConstructor, SecretData};

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FlagTarget {
	Item,
	Field,
}

#[derive(Clone)]
pub enum Param<T> {
	Default(T),
	Other(T),
}

impl<T> Param<T> {
	pub fn is_default(&self) -> bool {
		match self {
			Param::Default(_) => true,
			_ => false,
		}
	}
}

impl<T: ToTokens> ToTokens for Param<T> {
	fn to_tokens(&self, tokens: &mut TokenStream2) {
		match self {
			Param::Default(x) => x.to_tokens(tokens),
			Param::Other(x) => x.to_tokens(tokens),
		}
	}
}

#[derive(Clone)]
pub enum Function {
	Default,
	Serde(Ident),
	With(Path, Scope),
	Expr(Expr),
	As(Type, AsConversion),
	Secret {
		encryption: Option<SecretConstructor>,
		public_key: Option<Expr>,
		private_key: Option<Expr>,
	}
}

impl Function {
	pub fn is_default(&self) -> bool {
		match self {
			Self::Default => true,
			_ => false,
		}
	}
}

/// Represents all the possible modifiers a target can have. Use the apply() method to
/// have all the modifiers validated
#[derive(Clone)]
pub struct ModifierGroup {
	pub target: ModTarget,
	pub num_encoding: Option<NumEncoding>,
	pub endianness: Option<Endianness>,
	pub max: Option<Expr>,
	pub bit_width: Option<BitWidth>
}

impl ModifierGroup {
	pub fn new(target: ModTarget) -> Self {
		Self {
			target,
			num_encoding: None,
			endianness: None,
			max: None,
			bit_width: None,
		}
	}

	pub fn empty(&self) -> bool {
		self.num_encoding.is_none() &&
			self.endianness.is_none() &&
			self.max.is_none() &&
			self.bit_width.is_none()
	}

	pub fn apply(&mut self, modifier: Modifier) -> syn::Result<()> {
		const REPEATED_NUM_ENCODING: &str = "Num encoding modifier declared twice for the same target";
		const REPEATED_ENDIANNESS: &str = "Endianness modifier declared twice for the same target";
		const REPEATED_MAX: &str = "Max size modifier declared twice for the same target";
		const REPEATED_BIT_WIDTH: &str = "Bit width modifier declared twice for the same target";

		const ONLY_SIZE: &str = r#"This modifier can only be applied to the "size" target"#;
		const ONLY_VARIANT_AND_SIZE: &str = r#"This modifier can only be applied to the "size" and "variant" targets"#;

		match modifier {
			Modifier::Fixed { kw, .. } => {
				if self.num_encoding.is_some() {
					return Err(Error::new(kw.span(), REPEATED_NUM_ENCODING))
				}

				self.num_encoding = Some(NumEncoding::Fixed);
			}
			Modifier::Leb128 { kw, .. } => {
				if self.num_encoding.is_some() {
					return Err(Error::new(kw.span(), REPEATED_NUM_ENCODING))
				}

				self.num_encoding = Some(NumEncoding::Leb128);
			}
			Modifier::BigEndian { kw, .. } => {
				if self.endianness.is_some() {
					return Err(Error::new(kw.span(), REPEATED_ENDIANNESS))
				}

				self.endianness = Some(Endianness::BigEndian);
			}
			Modifier::LittleEndian { kw, .. } => {
				if self.endianness.is_some() {
					return Err(Error::new(kw.span(), REPEATED_ENDIANNESS))
				}

				self.endianness = Some(Endianness::LittleEndian);
			}
			Modifier::Max { kw, max, .. } => {
				if self.target.tier() < 2 {
					return Err(Error::new(kw.span(), ONLY_SIZE))
				}
				if self.max.is_some() {
					return Err(Error::new(kw.span(), REPEATED_MAX))
				}

				self.max = Some(max);
			}
			Modifier::BitWidth { lit, width, .. } => {
				if self.target.tier() < 1 {
					return Err(Error::new(lit.span(), ONLY_VARIANT_AND_SIZE))
				}
				if self.bit_width.is_some() {
					return Err(Error::new(lit.span(), REPEATED_BIT_WIDTH))
				}

				self.bit_width = Some(width);
			}
		}
		Ok(())
	}
}

/// Holds modifiers for each target
#[derive(Clone)]
pub struct AllModifiers {
	pub num: ModifierGroup,
	pub size: ModifierGroup,
	pub variant: ModifierGroup,
	pub flatten: Option<Expr>,
}

impl AllModifiers {
	pub fn new() -> Self {
		Self {
			num: ModifierGroup::new(ModTarget::Num { kw: Default::default() }),
			size: ModifierGroup::new(ModTarget::Size { kw: Default::default() }),
			variant: ModifierGroup::new(ModTarget::Variant { kw: Default::default() }),
			flatten: None,
		}
	}

	pub fn empty(&self) -> bool {
		self.num.empty() &&
			self.size.empty() &&
			self.variant.empty() &&
			self.flatten.is_none()
	}

	pub fn apply(&mut self, target: ModTarget, modifier: Modifier) -> syn::Result<()> {
		match target {
			ModTarget::Num { .. } => {
				self.num.target = target;
				self.num.apply(modifier)
			},
			ModTarget::Size { .. } => {
				self.size.target = target;
				self.size.apply(modifier)
			},
			ModTarget::Variant { .. } => {
				self.variant.target = target;
				self.variant.apply(modifier)
			},
		}
	}
}

/// A stream modifier - compression or encryption
#[derive(Clone)]
pub enum StreamModifier {
	Encrypted {
		encryption: Option<EncryptionConstructor>,
		key: Option<Expr>,
		iv: Option<Expr>,
	},
	Compressed {
		compression: Option<CompressionConstructor>,
	}
}

/// All the possible flags a field or item can have. The target allows the apply method to
/// check whether each flag is supported.
#[derive(Clone)]
pub struct Flags {
	/// Whether this is an item or field
	pub target: FlagTarget,
	/// The name of the crate - ende by default
	pub crate_name: Param<Ident>,
	/// Only set when the "skip" flag is specified. Will generate empty Encode and Decode
	/// implementations. Can only be accompanied by the "default" flag.
	pub skip: bool,
	/// The fallback used whenever a field or item can't be parsed for whatever reason
	pub default: Param<Expr>,
	/// The function used for encoding / decoding.
	pub function: Function,
	/// The modifiers apply to this item
	pub mods: AllModifiers,
	/// If present, indicates the item should be validated using the given expression before
	/// encoding and after decoding.
	pub validate: Option<(Expr, Option<Formatting>)>,
	/// If present indicates a field should only be encoded or decoded if the given condition
	/// is true. During decoding, if the condition is false, the default value will be used
	pub condition: Option<Expr>,
	/// Modifiers to the underlying Write/Read object itself. Indicate something should be
	/// encrypted or compressed before being encoded or decoded.
	pub stream_modifiers: Vec<StreamModifier>,
}

impl Flags {
	pub fn new(target: FlagTarget) -> Self {
		Self {
			target,
			crate_name: Param::Default(dollar_crate(ENDE)),
			skip: false,
			default: Param::Default(parse_quote!(Default::default())),
			function: Function::Default,
			mods: AllModifiers::new(),
			validate: None,
			condition: None,
			stream_modifiers: Vec::new(),
		}
	}

	pub fn skip_compatible(&self) -> bool {
		self.function.is_default() &&
			self.mods.empty() &&
			self.condition.is_none() &&
			self.stream_modifiers.is_empty()
	}
}

impl Flags {
	/// Applies a flag to the item, performing consistency checks.
	pub fn apply(&mut self, flag: Flag, scope: Scope) -> syn::Result<()> {
		let span = flag.span();
		match flag {
			Flag::Crate { crate_name, .. } => {
				if self.target == FlagTarget::Field {
					return Err(Error::new(span, r#""crate" flag can only be applied at the item level"#))
				}

				if !self.crate_name.is_default() {
					return Err(Error::new(span, r#""crate" flag declared more than once"#))
				}

				self.crate_name = Param::Other(crate_name);
			}
			Flag::Serde { crate_name ,.. } => {
				if !self.function.is_default() {
					return Err(Error::new(span, r#""serde" flag is incompatible with "as", "secret", "with", "expr" flags"#))
				}

				// If no name is specified, it is assumed to be "serde"
				let crate_name = crate_name
					.map(|(_, x)| x)
					.unwrap_or(Ident::new("serde", span));

				self.function = Function::Serde(crate_name);
			}
			Flag::Skip { .. } => {
				if self.skip {
					return Err(Error::new(span, r#""skip" flag declared more than once"#))
				}

				self.skip = true;
			}
			Flag::Default { expr, .. } => {
				if !self.default.is_default() {
					return Err(Error::new(span, r#""default" flag declared more than once"#))
				}

				self.default = Param::Other(expr);
			}
			Flag::With { path, .. } => {
				if !self.function.is_default() {
					return Err(Error::new(span, r#""with" flag is incompatible with "as", "secret", "serde", "expr" flags"#))
				}

				self.function = Function::With(path, scope);
			}
			Flag::Expr { expr, .. } => {
				if scope == Scope::Both {
					return Err(Error::new(span, r#""expr" flag must be scoped"#))
				}
				if !self.function.is_default() {
					return Err(Error::new(span, r#""expr" flag is incompatible with "as", "secret", "serde", "with" flags"#))
				}

				self.function = Function::Expr(expr);
			}
			Flag::As { ty, method, .. } => {
				if !self.function.is_default() {
					return Err(Error::new(span, r#""as" flag is incompatible with "with", "secret", "serde", "expr" flags"#))
				}

				self.function = Function::As(ty, method);
			}
			Flag::Flatten { expr, .. } => {
				if self.mods.flatten.is_some() {
					return Err(Error::new(span, r#""flatten" flag declared more than once"#))
				}

				let expr = expr.map(|x| x.1).unwrap_or(parse_quote!(1));
				self.mods.flatten = Some(expr);
			}
			Flag::Validate { expr, fmt, .. } => {
				if self.validate.is_some() {
					return Err(Error::new(span, r#""validate" flag declared more than once"#))
				}

				 self.validate = Some((expr, fmt.map(|x| x.1)));
			}
			Flag::Secret { data, .. } => {
				if !self.function.is_default() {
					return Err(Error::new(span, r#""secret" flag is incompatible with "with", "as", "serde" flags"#))
				}

				let data: Option<SecretData> = data.map(|x| x.1);

				// Validate and extract the parameters
				let mut encryption = None;
				let mut public_key = None;
				let mut private_key = None;

				if let Some(data) = data {
					let validated = data.validate()?;
					encryption = Some(validated.0);
					public_key = validated.1;
					private_key = validated.2;
				}

				self.function = Function::Secret {
					encryption,
					public_key,
					private_key,
				}
			}
			Flag::Encrypted { data, .. } => {
				let data: Option<EncryptionData> = data.map(|x| x.1);

				// Validate and extract the parameters
				let mut encryption = None;
				let mut key = None;
				let mut iv = None;

				if let Some(data) = data {
					let validated = data.validate()?;
					encryption = Some(validated.0);
					key = validated.1;
					iv = validated.2;
				}

				self.stream_modifiers.push(StreamModifier::Encrypted {
					encryption,
					key,
					iv,
				})
			}
			Flag::Compressed { data, .. } => {
				let compression = data.map(|x| x.1.ctor);

				self.stream_modifiers.push(StreamModifier::Compressed {
					compression,
				})
			}
			Flag::Modifiers { target, modifiers, .. } => {
				for modifier in modifiers {
					self.mods.apply(target.clone(), modifier)?;
				}
			}
			Flag::If { expr, .. } => {
				if self.target == FlagTarget::Item {
					return Err(Error::new(span, r#""if" flag can only be applied to fields"#))
				}

				if self.condition.is_some() {
					return Err(Error::new(span, r#""if" flag declared more than once"#))
				}

				self.condition = Some(expr);
			}
			Flag::En { .. } | Flag::De { .. } => {
				return Err(Error::new(span, r#"The flags "en" and "de" must be the first"#))
			}
		}

		if self.skip && !self.skip_compatible() {
			return Err(Error::new(span, r#""skip" flag can only be accompanied by "default" or "validate" flag"#))
		}

		Ok(())
	}
}