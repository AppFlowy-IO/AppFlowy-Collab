#![allow(clippy::all)]
#![allow(unused_attributes)]
#![allow(unused_assignments)]

use crate::internal::ctxt::ASTResult;
use proc_macro2::Ident;
use syn::Meta::NameValue;
use syn::{self, punctuated::Punctuated, Fields, Path, Token};

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
            }
            syn::Data::Union(_) => {
                ast_result.error_spanned_by(ast, "Does not support derive for unions");
                return None;
            }
            syn::Data::Enum(data) => {
                // https://docs.rs/syn/1.0.48/syn/struct.DataEnum.html
                ASTData::Enum(enum_from_ast(ast_result, &ast.ident, &data.variants))
            }
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
            }
            ASTData::Struct(_, fields) => Box::new(fields.iter()),
        }
    }

    pub fn all_idents(&'a self) -> Box<dyn Iterator<Item = &'a syn::Ident> + 'a> {
        match self {
            ASTData::Enum(variants) => Box::new(variants.iter().map(|v| &v.ident)),
            ASTData::Struct(_, fields) => {
                let iter = fields.iter().flat_map(|f| match &f.member {
                    syn::Member::Named(ident) => Some(ident),
                    _ => None,
                });
                Box::new(iter)
            }
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

impl<'a> ASTEnumVariant<'a> {
    pub fn name(&self) -> String {
        self.ident.to_string()
    }
}

pub struct ASTField<'a> {
    pub member: syn::Member,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

impl<'a> ASTField<'a> {
    pub fn new(_ctxt: &ASTResult, field: &'a syn::Field, index: usize) -> Result<Self, String> {
        Ok(ASTField {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(index.into()),
            },
            ty: &field.ty,
            original: field,
        })
    }

    pub fn name(&self) -> Option<syn::Ident> {
        if let syn::Member::Named(ident) = &self.member {
            Some(ident.clone())
        } else {
            None
        }
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
        }
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
    cx: &ASTResult,
    fields: &'a Punctuated<syn::Field, Token![,]>,
) -> Vec<ASTField<'a>> {
    fields
        .iter()
        .enumerate()
        .flat_map(|(index, field)| ASTField::new(cx, field, index).ok())
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
