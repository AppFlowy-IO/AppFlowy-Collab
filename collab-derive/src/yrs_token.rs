use crate::internal::{ASTContainer, ASTResult};
use proc_macro2::{Ident, TokenStream};

use syn::{AngleBracketedGenericArguments, PathSegment, Type};

pub fn make_yrs_token_steam(ast_result: &ASTResult, ast: &ASTContainer) -> Option<TokenStream> {
  let map_token_stream = token_stream_for_yrs_map(ast_result, ast);
  let token_stream: TokenStream = quote! {
      #map_token_stream


  };
  Some(token_stream)
}

fn token_stream_for_yrs_map(ast_result: &ASTResult, ast: &ASTContainer) -> Option<TokenStream> {
  let struct_name = ast.ident.clone();
  let struct_map_modifier = format_ident!("{}MapRef", struct_name.to_string());
  let setter_getter_stream_token = ast
    .data
    .all_fields()
    .flat_map(|field| setter_getter_token_stream(ast_result, &field.member, field.ty));

  let into_inner_token_stream = ast
    .data
    .all_fields()
    .flat_map(|field| into_inner_token_stream(ast_result, &field.member, field.ty));

  Some(quote! {
      pub struct #struct_map_modifier {
          map_ref: collab::preclude::MapRefWrapper,
      }

      impl #struct_map_modifier {
          pub fn new(map_ref: collab::preclude::MapRefWrapper) -> Self {
              Self { map_ref }
          }

          #(#setter_getter_stream_token)*

          pub fn into_object(&self, txn: &collab::preclude::Transaction) -> #struct_name {
              #struct_name {
                  #(#into_inner_token_stream)*
              }
          }
      }

      impl collab::preclude::CustomMapRef for #struct_map_modifier {
          fn from_map_ref(map_ref: collab::preclude::MapRefWrapper) -> Self {
              Self { map_ref}
          }
      }

      impl std::ops::Deref for #struct_map_modifier {
          type Target = collab::preclude::MapRefWrapper;
          fn deref(&self) -> &Self::Target {
              &self.map_ref
          }
      }

  })
}

fn into_inner_token_stream(
  ast_result: &ASTResult,
  member: &syn::Member,
  ty: &Type,
) -> Option<TokenStream> {
  let ident_type = IdentType::from_ty(ast_result, ty);
  into_inner_field_token_stream(ast_result, member, ty, &ident_type, false)
}

fn into_inner_field_token_stream(
  ast_result: &ASTResult,
  member: &syn::Member,
  ty: &Type,
  ident_type: &IdentType,
  is_option: bool,
) -> Option<TokenStream> {
  let ident = get_member_ident(ast_result, member)?;
  let getter = format_ident!("get_{}", ident.to_string());
  match ident_type {
    IdentType::StringType
    | IdentType::I64Type
    | IdentType::F64Type
    | IdentType::BoolType
    | IdentType::ArrayType { .. }
    | IdentType::HashMapType { .. } => {
      if is_option {
        Some(quote! {
            #ident: self.#getter(txn),
        })
      } else {
        Some(quote! {
            #ident: self.#getter(txn).unwrap_or_default(),
        })
      }
    },
    IdentType::Others => Some(quote! {
       #ident: self.#getter::<#ty>(txn).unwrap_or_default(),
    }),
    IdentType::OptionType {
      ident_type,
      inner_ty,
    } => into_inner_field_token_stream(ast_result, member, inner_ty, ident_type, true),
  }
}

fn setter_getter_token_steam_for_item_type(
  key: String,
  setter: Ident,
  getter: Ident,
  ty: &Type,
  ident: &Ident,
  ident_type: &IdentType,
) -> Option<TokenStream> {
  match ident_type {
    IdentType::StringType => Some(quote! {
        pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
            self.map_ref.insert_with_txn(txn, #key, value)
        }
        pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
            self.map_ref.get_str_with_txn(txn, #key)
        }
    }),
    IdentType::I64Type => Some(quote! {
        pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
            self.map_ref.insert_with_txn(txn, #key, value)
        }
        pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
            self.map_ref.get_i64_with_txn(txn, #key)
        }
    }),
    IdentType::F64Type => Some(quote! {
        pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
            self.map_ref.insert_with_txn(txn, #key, value)
        }
        pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
            self.map_ref.get_f64_with_txn(txn, #key)
        }
    }),
    IdentType::BoolType => Some(quote! {
        pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
            self.map_ref.insert_with_txn(txn, #key, value)
        }
        pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
            self.map_ref.get_bool_with_txn(txn, #key)
        }
    }),
    IdentType::HashMapType { value_type } => {
      let update = format_ident!("update_{}_key_value", ident.to_string());
      Some(quote! {
          pub fn #update(&mut self, txn: &mut collab::preclude::TransactionMut, key: &str, value: #value_type) {
              if let Some(map_ref) = self.map_ref.get_map_with_txn(txn, #key) {
                  map_ref.insert_with_txn(txn, key, value);
              }
          }

          pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
              self.map_ref.insert_json_with_txn(txn, #key, value)
          }

          pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
              self.map_ref.get_json_with_txn(txn, #key)
          }
      })
    },
    IdentType::Others => Some(quote! {
        pub fn #setter<T: serde::Serialize>(&mut self, txn: &mut collab::preclude::TransactionMut, value: T) {
            self.map_ref.insert_json_with_txn(txn, #key, value);
        }

        pub fn #getter<T: serde::de::DeserializeOwned>(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
            self.map_ref.get_json_with_txn::<#ty>(txn, #key)
        }
    }),
    IdentType::OptionType {
      ident_type,
      inner_ty,
    } => setter_getter_token_steam_for_item_type(key, setter, getter, inner_ty, ident, ident_type),
    IdentType::ArrayType {
      ident_type: _,
      inner_ty: _,
    } => Some(quote! {
          pub fn #setter(&mut self, txn: &mut collab::preclude::TransactionMut, value: #ty) {
              self.map_ref.insert_json_with_txn(txn, #key, value)
          }

          pub fn #getter(&self, txn: &collab::preclude::Transaction) -> Option<#ty> {
              self.map_ref.get_json_with_txn(txn, #key)
          }
    }),
  }
}
fn setter_getter_token_stream(
  ast_result: &ASTResult,
  member: &syn::Member,
  ty: &Type,
) -> Option<TokenStream> {
  let ident = get_member_ident(ast_result, member)?;
  let key = ident.to_string();
  let setter = format_ident!("set_{}", ident.to_string());
  let getter = format_ident!("get_{}", ident.to_string());
  let ident_type = IdentType::from_ty(ast_result, ty);
  setter_getter_token_steam_for_item_type(key, setter, getter, ty, ident, &ident_type)
}

pub(crate) fn get_member_ident<'a>(
  ast_result: &ASTResult,
  member: &'a syn::Member,
) -> Option<&'a syn::Ident> {
  if let syn::Member::Named(ref ident) = member {
    Some(ident)
  } else {
    ast_result.error_spanned_by(
      member,
      "Unsupported member, shouldn't be self.0".to_string(),
    );
    None
  }
}

#[derive(Debug, Eq, PartialEq)]
enum IdentType {
  StringType,
  I64Type,
  F64Type,
  BoolType,
  HashMapType {
    value_type: Ident,
  },
  OptionType {
    ident_type: Box<IdentType>,
    inner_ty: Type,
  },
  ArrayType {
    ident_type: Box<IdentType>,
    inner_ty: Type,
  },
  Others,
}

impl IdentType {
  pub fn from_ty(ast_result: &ASTResult, ty: &Type) -> Self {
    if let Type::Path(p) = &ty {
      let mut ident_type = match p.path.get_ident() {
        None => IdentType::Others,
        Some(ident) => match ident.to_string().as_ref() {
          "String" => IdentType::StringType,
          "bool" => IdentType::BoolType,
          "i64" => IdentType::I64Type,
          "f64" => IdentType::F64Type,
          _ => IdentType::Others,
        },
      };

      if ident_type == IdentType::Others {
        if let Some(seg) = p.path.segments.last() {
          if seg.ident == "HashMap" {
            let types = get_bracketed_value_type_from(ast_result, seg);
            let ident = parse_ty(types[1]).unwrap();
            ident_type = IdentType::HashMapType { value_type: ident };
          }

          if seg.ident == "Vec" {
            let types = get_bracketed_value_type_from(ast_result, seg);
            let item_type = IdentType::from_ty(ast_result, types[0]);
            ident_type = IdentType::ArrayType {
              ident_type: Box::new(item_type),
              inner_ty: types[0].clone(),
            };
          }

          if seg.ident == "Option" {
            let types = get_bracketed_value_type_from(ast_result, seg);
            let item_type = IdentType::from_ty(ast_result, types[0]);
            ident_type = IdentType::OptionType {
              ident_type: Box::new(item_type),
              inner_ty: types[0].clone(),
            };

            // if let Type::Path(p) = types[0] {
            //     let inner_ty = p.path.get_ident().cloned().unwrap();
            //     let item_type = IdentType::from_ty(ast_result, types[0]);
            //     ident_type = IdentType::OptionType {
            //         ident_type: Box::new(item_type),
            //         inner_ty: types[0].clone(),
            //     };
            // } else {
            //     ast_result.error_spanned_by(
            //         types[0],
            //         "Can not infer the bracket inner type of the Option",
            //     );
            //     IdentType::Others
            // };
          }
        }
      }
      ident_type
    } else {
      IdentType::Others
    }
  }
}

fn get_bracketed_value_type_from<'a>(
  ast_result: &ASTResult,
  seg: &'a PathSegment,
) -> Vec<&'a Type> {
  if let syn::PathArguments::AngleBracketed(ref bracketed) = seg.arguments {
    return match seg.ident.to_string().as_ref() {
      "HashMap" => parse_bracketed(bracketed),
      "Vec" => parse_bracketed(bracketed),
      "Option" => parse_bracketed(bracketed),
      _ => {
        let msg = format!("Unsupported type: {}", seg.ident);
        ast_result.error_spanned_by(&seg.ident, msg);
        vec![]
      },
    };
  }
  vec![]
}

fn parse_bracketed(bracketed: &AngleBracketedGenericArguments) -> Vec<&Type> {
  bracketed
    .args
    .iter()
    .flat_map(|arg| {
      if let syn::GenericArgument::Type(ref ty_in_bracket) = arg {
        Some(ty_in_bracket)
      } else {
        None
      }
    })
    .collect::<Vec<&syn::Type>>()
}

fn parse_ty(ty: &Type) -> Option<Ident> {
  if let Type::Path(ref p) = ty {
    if p.path.segments.len() != 1 {
      return None;
    }

    return match p.path.segments.last() {
      Some(seg) => Some(seg.ident.clone()),
      None => return None,
    };
  }
  None
}
