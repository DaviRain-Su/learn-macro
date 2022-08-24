use std::iter::Map;
use std::slice::Iter;

use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::quote;
use syn::GenericArgument;
use syn::Path;
use syn::PathArguments;
use syn::Type;
use syn::TypePath;
use darling::FromField;
use syn::{
    Data, DataStruct, DeriveInput, Fields, FieldsNamed,
};

type TokenStreamIter<'a> = Map<Iter<'a, Fd>, fn(&'a Fd) -> TokenStream>;

#[derive(Debug, Default, FromField)]
#[darling(default, attributes(builder))]
struct Opts {
    each: Option<String>,
    default: Option<String>,
}

#[derive(Debug)]
struct Fd {
    name: Ident,
    ty: Type,
    opts: Opts,
}


#[derive(Debug)]
pub struct BuilderContext {
    name: Ident,
    fields: Vec<Fd>,
}

impl BuilderContext {
    pub fn new(input: DeriveInput) -> Self {
        let name = input.ident;
        let fields = if let Data::Struct(DataStruct {
            fields: Fields::Named(FieldsNamed { named, .. }),
            ..
        }) = input.data
        {
            named
        } else {
            panic!("Unsupported data type");
        };

        let fds = fields.into_iter().map(|f| {
            Fd {
                opts: Opts::from_field(&f).unwrap_or_default(),
                name: f.ident.unwrap(),
                ty: f.ty,
            }
        }).collect();
   
        Self { name, fields: fds }
    }

    pub fn generate(&self) -> TokenStream {
        let name = &self.name;
        // builder name: {}Builder, e.g.CommandBuilder
        let builder_name = Ident::new(&format!("{}Builder", name), name.span());
        // option filels. e.g. executable: String -> executable: Option<String>
        let optionized_fields = self.gen_optionized_fields();
        // method: fn executable(mut self, v: impl Into<String>) -> Self { self.executable = Some(v); self}
        // Command::Builder().executable("hello").args(vec![]).envs(vec![]).finish()
        let methods = self.gen_methods();
        // assign build fileds back to origin struct fields
        // field_name: self.#field_name.take().ok_or(" xx need to be set!")
        let assigns = self.gen_assigns();

        quote! {
            /// Builder structure
            #[derive(Debug, Default)]
            struct #builder_name {
                #(#optionized_fields,)*
            }

            impl #builder_name {
                #(#methods)*

                pub fn finish(mut self) -> Result<#name, &'static str> {
                    Ok(#name {
                        #(#assigns,)*
                    })
                }

            }

            impl #name {
                fn builder() -> #builder_name {
                    Default::default()
                }
            }
        }
    }

    fn gen_optionized_fields(&self) -> TokenStreamIter {
        self.fields.iter().map(|f| {
            
            let (_, ty) = get_option_inner(&f.ty);
            let name = &f.name;
            quote! { #name: std::option::Option<#ty> }
        })
    }

    fn gen_methods(&self) -> TokenStreamIter {
        self.fields.iter().map(|f| {
            let (_, ty) = get_option_inner(&f.ty);
            let (is_vec, vec_inner_type) = get_vec_inner(&f.ty);
            let name = &f.name;
            if is_vec {
                if let Some(each_name) = f.opts.each.as_deref() {
                    let each_name = Ident::new(each_name, f.name.span());
                    return   quote! {
                        pub fn #each_name(mut self, v: impl Into<#vec_inner_type>) -> Self { 
                            let mut data = self.#name.take().unwrap_or_default();
                            data.push(v.into());
                            self.#name = Some(data);
                            self
                        }
                    };
                }
            }

            // option fields. e.g. executable: String -> executable: Option<String>
            quote! {
                pub fn #name(mut self, v: impl Into<#ty>) -> Self {
                    self.#name = Some(v.into());
                    self
                }
            }
        })
    }

    fn gen_assigns(&self) -> TokenStreamIter {
        self.fields.iter().map(|f| {
            let name = &f.name;
            let (optional, _) = get_option_inner(&f.ty);
            if optional {
                return quote! {
                    #name: self.#name.take()
                };
            }

            if let Some(default) = f.opts.default.as_deref() {
                let ast : TokenStream = default.parse().unwrap();
                return quote! { #name: self.#name.take().unwrap_or_else(|| #ast)}
            }

            // field_name: self.#field_name.take().ok_or(" xx need to be set!")
            quote! {
                #name: self.#name.take().ok_or(concat!(stringify!(#name), " needs to be set!"))?
            }
        })
    }
}

fn get_option_inner(ty: &Type) -> (bool, &Type) {
    get_type_inner(ty, "Option")
}


fn get_vec_inner(ty: &Type) -> (bool, &Type) {
    get_type_inner(ty, "Vec")
}


fn get_type_inner<'a>(ty: &'a Type, name: &str) -> (bool, &'a Type) {
    if let Type::Path(TypePath { path: Path {segments, ..}, ..}) = ty {
        if let Some(v) = segments.first() {
            if v.ident == name {
                let t = match &v.arguments {
                    PathArguments::AngleBracketed(a) => match a.args.iter().next() {
                        Some(GenericArgument::Type(t)) => t,
                        _ => panic!("Not sure what to do with other GenericArgument"),
                    },
                    _ => panic!("Not sure what to do with other PathArguments"),
                };
                return (true, t);   
            }
        }
    }
    
    return (false, ty);
}