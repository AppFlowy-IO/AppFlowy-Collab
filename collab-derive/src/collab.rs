use crate::internal::{ASTContainer, ASTResult};
use crate::yrs_token::make_yrs_token_steam;
use proc_macro2::TokenStream;

pub fn expand_derive(input: &syn::DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
  let ast_result = ASTResult::new();
  let cont = match ASTContainer::from_ast(&ast_result, input) {
    Some(cont) => cont,
    None => return Err(ast_result.check().unwrap_err()),
  };

  let mut token_stream: TokenStream = TokenStream::default();
  if let Some(yrs_token_stream) = make_yrs_token_steam(&ast_result, &cont) {
    token_stream.extend(yrs_token_stream);
  }

  ast_result.check()?;
  Ok(token_stream)
}
