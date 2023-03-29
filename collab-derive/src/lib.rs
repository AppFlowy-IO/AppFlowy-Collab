mod collab;
mod internal;
mod yrs_token;

#[macro_use]
extern crate quote;
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::DeriveInput;

#[proc_macro_derive(Collab, attributes(collab, collab_key))]
pub fn derive_collab(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  collab::expand_derive(&input)
    .unwrap_or_else(to_compile_errors)
    .into()
}

fn to_compile_errors(errors: Vec<syn::Error>) -> proc_macro2::TokenStream {
  let compile_errors = errors.iter().map(syn::Error::to_compile_error);
  quote!(#(#compile_errors)*)
}
