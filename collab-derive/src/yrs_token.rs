use crate::internal::{ASTContainer, ASTResult};
use proc_macro2::{Ident, TokenStream};

pub fn make_yrs_token_steam(ast_result: &ASTResult, ast: &ASTContainer) -> Option<TokenStream> {
    let build_take_fields = ast
        .data
        .all_fields()
        .flat_map(|field| token_stream_for_field(ast_result, &field.member, field.ty));

    let map_token_stream = token_stream_for_yrs_map(ast_result, ast);
    let token_stream: TokenStream = quote! {
        #map_token_stream

        #(#build_take_fields)*

    };
    Some(token_stream)
}

fn token_stream_for_field(
    ast_result: &ASTResult,
    member: &syn::Member,
    ty: &syn::Type,
) -> Option<TokenStream> {
    let ident = get_member_ident(ast_result, member)?;
    eprintln!("😄{}", ident);
    None
}

fn token_stream_for_yrs_map(ast_result: &ASTResult, ast: &ASTContainer) -> Option<TokenStream> {
    let mut key =
        ast.data
            .all_fields()
            .find(|field| field.name().is_some())
            .map(|field| field.name().unwrap())
            .unwrap_or_else(|| {
                format_ident!("{}",ast.path.clone().expect(
                "Can't find the id or the key defined by #[collab_key = \"xx\" in the struct",
            ))
            });

    let struct_name = ast.ident.clone();
    Some(quote! {
        impl #struct_name {
            pub fn insert_into_parent(&self, parent: yrs::MapRef, txn: &mut yrs::TransactionMut) -> yrs::MapRef {
                let map = yrs::MapPrelim::<lib0::any::Any>::new();
                parent.insert(txn, self.#key.as_str(), map)
            }

            pub fn map_modifier_from(&self, parent: yrs::MapRef, collab_transact: collab::CollabTransact) -> Option<collab::MapModifier> {
                let txn = collab_transact.transact();
                let map = parent.get(&txn, self.#key.as_str()).map(|value| value.to_ymap())??;
                drop(txn);
                Some(collab::MapModifier::new(collab_transact, map))
            }
        }
    })
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
