#![allow(clippy::all)]
#![allow(unused_attributes)]
#![allow(unused_assignments)]

use crate::internal::ctxt::ASTResult;
use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use std::fmt;
use std::fmt::Display;
use syn::Meta::{List, NameValue};
use syn::NestedMeta::Meta;
use syn::{self, punctuated::Punctuated, Fields, LitStr, Path, Token};

pub struct ASTContainer<'a> {
  /// The struct or enum name (without generics).
  pub ident: syn::Ident,

  pub path: Option<String>,

  /// The contents of the struct or enum.
  pub data: ASTData<'a>,
}

impl<'a> ASTContainer<'a> {
  pub fn from_ast(ast_result: &ASTResult, ast: &'a syn::DeriveInput) -> Option<ASTContainer<'a>> {
    let data = match &ast.data {
      syn::Data::Struct(data) => {
        // https://docs.rs/syn/1.0.48/syn/struct.DataStruct.html
        let (style, fields) = struct_from_ast(ast_result, &data.fields);
        ASTData::Struct(style, fields)
      },
      syn::Data::Union(_) => {
        ast_result.error_spanned_by(ast, "Does not support derive for unions");
        return None;
      },
      syn::Data::Enum(data) => {
        // https://docs.rs/syn/1.0.48/syn/struct.DataEnum.html
        ASTData::Enum(enum_from_ast(ast_result, &ast.ident, &data.variants))
      },
    };

    let ident = ast.ident.clone();
    let path = get_key(ast_result, &ident, &ast.attrs);
    let item = ASTContainer { ident, path, data };
    Some(item)
  }
}

pub enum ASTData<'a> {
  Struct(ASTStyle, Vec<ASTField<'a>>),
  Enum(Vec<ASTEnumVariant<'a>>),
}

impl<'a> ASTData<'a> {
  pub fn all_fields(&'a self) -> Box<dyn Iterator<Item = &'a ASTField<'a>> + 'a> {
    match self {
      ASTData::Enum(variants) => {
        Box::new(variants.iter().flat_map(|variant| variant.fields.iter()))
      },
      ASTData::Struct(_, fields) => Box::new(fields.iter()),
    }
  }
}

/// A variant of an enum.
pub struct ASTEnumVariant<'a> {
  pub ident: syn::Ident,
  pub style: ASTStyle,
  pub fields: Vec<ASTField<'a>>,
  pub original: &'a syn::Variant,
}

pub struct ASTField<'a> {
  pub member: syn::Member,
  pub ty: &'a syn::Type,
  pub yrs_attr: YrsAttribute,
  pub original: &'a syn::Field,
}

impl<'a> ASTField<'a> {
  pub fn new(ast_result: &ASTResult, field: &'a syn::Field, index: usize) -> Result<Self, String> {
    Ok(ASTField {
      member: match &field.ident {
        Some(ident) => syn::Member::Named(ident.clone()),
        None => syn::Member::Unnamed(index.into()),
      },
      ty: &field.ty,
      yrs_attr: YrsAttribute::from_ast(ast_result, field),
      original: field,
    })
  }
}

pub const YRS: Symbol = Symbol("yrs");
pub const PRS_TY: Symbol = Symbol("ty");
pub struct YrsAttribute {
  #[allow(dead_code)]
  ty: Option<LitStr>,
}

impl YrsAttribute {
  /// Extract out the `#[yrs(...)]` attributes from a struct field.
  pub fn from_ast(ast_result: &ASTResult, field: &syn::Field) -> Self {
    let mut ty = ASTFieldAttr::none(ast_result, PRS_TY);
    for meta_item in field
      .attrs
      .iter()
      .flat_map(|attr| get_yrs_nested_meta(ast_result, attr))
      .flatten()
    {
      match &meta_item {
        // Parse '#[yrs(ty = x)]'
        Meta(NameValue(m)) if m.path == PRS_TY => {
          if let syn::Lit::Str(lit) = &m.lit {
            ty.set(&m.path, lit.clone());
          }
        },

        _ => {
          ast_result.error_spanned_by(meta_item, "unexpected meta in field attribute");
        },
      }
    }
    YrsAttribute { ty: ty.get() }
  }
}

fn get_yrs_nested_meta(cx: &ASTResult, attr: &syn::Attribute) -> Result<Vec<syn::NestedMeta>, ()> {
  // Only handle the attribute that we have defined
  if attr.path != YRS {
    return Ok(vec![]);
  }

  match attr.parse_meta() {
    Ok(List(meta)) => Ok(meta.nested.into_iter().collect()),
    Ok(_) => Ok(vec![]),
    Err(err) => {
      cx.error_spanned_by(attr, "attribute must be str, e.g. #[yrs(xx = \"xxx\")]");
      cx.syn_error(err);
      Err(())
    },
  }
}

pub struct ASTFieldAttr<'c, T> {
  ast_result: &'c ASTResult,
  name: Symbol,
  tokens: TokenStream,
  value: Option<T>,
}

impl<'c, T> ASTFieldAttr<'c, T> {
  pub(crate) fn none(ast_result: &'c ASTResult, name: Symbol) -> Self {
    ASTFieldAttr {
      ast_result,
      name,
      tokens: TokenStream::new(),
      value: None,
    }
  }

  pub(crate) fn set<A: ToTokens>(&mut self, obj: A, value: T) {
    let tokens = obj.into_token_stream();
    if self.value.is_some() {
      self
        .ast_result
        .error_spanned_by(tokens, format!("duplicate attribute `{}`", self.name));
    } else {
      self.tokens = tokens;
      self.value = Some(value);
    }
  }

  pub(crate) fn get(self) -> Option<T> {
    self.value
  }
}

#[derive(Copy, Clone)]
pub enum ASTStyle {
  Struct,
  Tuple,
  NewType,
  Unit,
}

pub fn struct_from_ast<'a>(cx: &ASTResult, fields: &'a Fields) -> (ASTStyle, Vec<ASTField<'a>>) {
  match fields {
    syn::Fields::Named(fields) => (ASTStyle::Struct, fields_from_ast(cx, &fields.named)),
    syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
      (ASTStyle::NewType, fields_from_ast(cx, &fields.unnamed))
    },
    syn::Fields::Unnamed(fields) => (ASTStyle::Tuple, fields_from_ast(cx, &fields.unnamed)),
    syn::Fields::Unit => (ASTStyle::Unit, Vec::new()),
  }
}

fn enum_from_ast<'a>(
  cx: &ASTResult,
  _ident: &Ident,
  variants: &'a Punctuated<syn::Variant, Token![,]>,
) -> Vec<ASTEnumVariant<'a>> {
  variants
    .iter()
    .flat_map(|variant| {
      let (style, fields) = struct_from_ast(cx, &variant.fields);
      Some(ASTEnumVariant {
        ident: variant.ident.clone(),
        style,
        fields,
        original: variant,
      })
    })
    .collect()
}

fn fields_from_ast<'a>(
  ast_result: &ASTResult,
  fields: &'a Punctuated<syn::Field, Token![,]>,
) -> Vec<ASTField<'a>> {
  fields
    .iter()
    .enumerate()
    .flat_map(|(index, field)| ASTField::new(ast_result, field, index).ok())
    .collect()
}

#[derive(Copy, Clone)]
pub struct Symbol(&'static str);

impl PartialEq<Symbol> for Ident {
  fn eq(&self, word: &Symbol) -> bool {
    self == word.0
  }
}

impl<'a> PartialEq<Symbol> for &'a Ident {
  fn eq(&self, word: &Symbol) -> bool {
    *self == word.0
  }
}

impl PartialEq<Symbol> for Path {
  fn eq(&self, word: &Symbol) -> bool {
    self.is_ident(word.0)
  }
}

impl<'a> PartialEq<Symbol> for &'a Path {
  fn eq(&self, word: &Symbol) -> bool {
    self.is_ident(word.0)
  }
}

impl Display for Symbol {
  fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str(self.0)
  }
}

pub const KEY: Symbol = Symbol("collab_key");

fn get_key(
  ast_result: &ASTResult,
  struct_name: &Ident,
  attrs: &[syn::Attribute],
) -> Option<String> {
  let mut key = None;
  attrs
    .iter()
    .filter(|attr| attr.path.segments.iter().any(|s| s.ident == KEY))
    .for_each(|attr| {
      if let Ok(NameValue(named_value)) = attr.parse_meta() {
        if key.is_some() {
          ast_result.error_spanned_by(struct_name, "Duplicate key type definition");
        }
        if let syn::Lit::Str(s) = named_value.lit {
          key = Some(s.value());
        }
      }
    });
  key
}
