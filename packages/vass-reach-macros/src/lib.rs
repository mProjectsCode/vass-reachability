use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, GenericArgument, Ident, PathArguments, Token, Type, Visibility,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token,
};

struct ConfigField {
    name: Ident,
    ty: Type,
    default_value: Expr,
    partial_ty: Option<Type>,
}

impl Parse for ConfigField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;

        // Support syntax:
        // field: Type = DefaultValue
        // field: Type (PartialType = DefaultValue)
        if input.peek(token::Paren) {
            let content;
            syn::parenthesized!(content in input);
            let partial_ty: Type = content.parse()?;
            content.parse::<Token![=]>()?;
            let default_value: Expr = content.parse()?;
            Ok(ConfigField {
                name,
                ty,
                default_value,
                partial_ty: Some(partial_ty),
            })
        } else {
            input.parse::<Token![=]>()?;
            let default_value: Expr = input.parse()?;
            Ok(ConfigField {
                name,
                ty,
                default_value,
                partial_ty: None,
            })
        }
    }
}

struct ConfigInput {
    vis: Visibility,
    name: Ident,
    fields: Punctuated<ConfigField, Token![,]>,
}

impl Parse for ConfigInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;
        let content;
        syn::braced!(content in input);
        let fields = content.parse_terminated(ConfigField::parse, Token![,])?;
        Ok(ConfigInput { vis, name, fields })
    }
}

/// Heuristic to check if a type is Option<T> to avoid double wrapping.
fn is_option(ty: &Type) -> bool {
    if let Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
    {
        return args.args.len() == 1 && matches!(args.args[0], GenericArgument::Type(_));
    }
    false
}

#[proc_macro]
pub fn config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ConfigInput);
    let vis = &input.vis;
    let struct_name = &input.name;
    let partial_struct_name = format_ident!("Partial{}", struct_name);

    let fields = input.fields.iter().map(|f| {
        let name = &f.name;
        let ty = &f.ty;
        quote! { #name: #ty }
    });

    let partial_fields = input.fields.iter().map(|f| {
        let name = &f.name;
        let ty = &f.ty;
        let final_partial_ty = if let Some(pt) = &f.partial_ty {
            quote! { #pt }
        } else if is_option(ty) {
            quote! { #ty }
        } else {
            quote! { Option<#ty> }
        };
        quote! { #name: #final_partial_ty }
    });

    let from_partial_assignments = input.fields.iter().map(|f| {
        let name = &f.name;
        let default_value = &f.default_value;
        quote! { #name: partial.#name.into_or(#default_value) }
    });

    let methods = input.fields.iter().map(|f| {
        let name = &f.name;
        let ty = &f.ty;
        let with_name = format_ident!("with_{}", name);
        let set_name = format_ident!("set_{}", name);
        let get_name = format_ident!("get_{}", name);
        quote! {
            pub fn #with_name(mut self, #name: #ty) -> Self {
                self.#name = #name;
                self
            }
            pub fn #set_name(&mut self, #name: #ty) {
                self.#name = #name;
            }
            pub fn #get_name(&self) -> &#ty {
                &self.#name
            }
        }
    });

    let default_assignments = input.fields.iter().map(|f| {
        let name = &f.name;
        let default_value = &f.default_value;
        quote! { #name: #default_value }
    });

    let expanded = quote! {
        #[derive(Debug, Clone, serde::Serialize)]
        #vis struct #struct_name {
            #( #fields, )*
        }

        #[derive(Debug, Clone, serde::Deserialize)]
        #vis struct #partial_struct_name {
            #( #partial_fields, )*
        }

        impl #struct_name {
            pub fn from_partial(partial: #partial_struct_name) -> Self {
                use crate::config::IntoOr;
                Self {
                    #( #from_partial_assignments, )*
                }
            }
            pub fn from_file<P: AsRef<std::path::Path>>(file_path: P) -> anyhow::Result<Self> {
                let canonic_path = std::fs::canonicalize(file_path)?;
                let content = std::fs::read_to_string(canonic_path)?;
                Ok(Self::from_partial(toml::from_str(&content)?))
            }
            pub fn from_optional_file<P: AsRef<std::path::Path>>(file_path: Option<P>) -> anyhow::Result<Self> {
                match file_path {
                    Some(p) => Self::from_file(p),
                    None => Ok(Self::default())
                }
            }
            #( #methods )*
        }

        impl Default for #struct_name {
            fn default() -> Self {
                #struct_name {
                    #( #default_assignments, )*
                }
            }
        }

        impl crate::config::IntoOr<#struct_name> for Option<#partial_struct_name> {
            fn into_or(self, or: #struct_name) -> #struct_name {
                match self {
                    Some(t) => #struct_name::from_partial(t),
                    None => or
                }
            }
        }
    };
    TokenStream::from(expanded)
}
