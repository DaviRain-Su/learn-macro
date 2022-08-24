mod builder;

use crate::builder::BuilderContext;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    println!("{:#?}", input);
    let context = BuilderContext::new(input);
    context.generate().into()
}
