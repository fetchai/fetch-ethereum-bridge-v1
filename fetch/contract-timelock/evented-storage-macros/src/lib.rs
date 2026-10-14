//! Attribute macros that turn a `cw_storage_plus` `Map`/`Item` declaration into an
//! evented storage wrapper. All behaviour lives in the runtime types
//! (`EventedMap`/`EventedItem`); these macros only derive the event names and
//! rewrite the declaration.

use proc_macro::TokenStream;
use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::parse::Parser;
use syn::{Expr, ExprLit, GenericArgument, ItemConst, Lit, LitStr, PathArguments, Type};

/// Path of the runtime support module, resolved in the crate using the macros.
/// This is the single place to change when the module moves into `cw-fetch-std`.
fn runtime() -> TokenStream2 {
    quote!(crate::evented_storage)
}

/// Wraps a `Map<K, V>` constant so that it can only be mutated through `update`/`remove`,
/// both of which emit events.
///
/// ```ignore
/// #[evented_map(key = "id", value = "message")]
/// const MESSAGE_QUEUE: Map<u64, BridgeMessage> = Map::new("message_queue");
/// ```
///
/// Arguments:
/// - `key` (required): attribute name, or a tuple of names for a composite key,
///   e.g. `key = ("role", "address")` for `Map<(&str, &Addr), ()>`.
/// - `value` (optional): attribute name of the value in the updated event. Omit it for
///   set-like maps.
/// - `updated` / `removed` (optional): override the derived event names
///   (`<const_name>_updated` / `<const_name>_removed`).
#[proc_macro_attribute]
pub fn evented_map(args: TokenStream, input: TokenStream) -> TokenStream {
    expand(args, input, Kind::Map)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Wraps an `Item<V>` constant so that it can only be mutated through `update`/`remove`,
/// both of which emit events.
///
/// ```ignore
/// #[evented_item(value = "nonce")]
/// const GLOBAL_NONCE: Item<u64> = Item::new("nonce");
/// ```
///
/// Arguments: `value` (required), `updated` / `removed` (optional event name overrides).
#[proc_macro_attribute]
pub fn evented_item(args: TokenStream, input: TokenStream) -> TokenStream {
    expand(args, input, Kind::Item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Map,
    Item,
}

#[derive(Default)]
struct Args {
    key: Option<Vec<LitStr>>,
    value: Option<LitStr>,
    updated: Option<LitStr>,
    removed: Option<LitStr>,
}

fn expand(args: TokenStream, input: TokenStream, kind: Kind) -> syn::Result<TokenStream2> {
    let item: ItemConst = syn::parse(input)?;
    let args = parse_args(args, kind)?;

    let ItemConst {
        attrs,
        vis,
        ident,
        ty,
        expr,
        ..
    } = item;

    let base_name = ident.to_string().to_lowercase();
    let updated = args
        .updated
        .map(|name| name.value())
        .unwrap_or_else(|| format!("{base_name}_updated"));
    let removed = args
        .removed
        .map(|name| name.value())
        .unwrap_or_else(|| format!("{base_name}_removed"));

    let rt = runtime();
    let expanded = match kind {
        Kind::Map => {
            let [key_ty, value_ty] = type_args::<2>(&ty, "Map")?;
            let key_names = args.key.unwrap_or_default();
            let arity = Literal::usize_unsuffixed(key_names.len());
            let value_name = match args.value {
                Some(name) => quote!(::core::option::Option::Some(#name)),
                None => quote!(::core::option::Option::None),
            };
            quote! {
                #(#attrs)*
                #vis const #ident: #rt::EventedMap<#key_ty, #value_ty, #arity> =
                    #rt::EventedMap::new(#expr, #updated, #removed, [#(#key_names),*], #value_name);
            }
        }
        Kind::Item => {
            let [value_ty] = type_args::<1>(&ty, "Item")?;
            let value_name = args.value;
            quote! {
                #(#attrs)*
                #vis const #ident: #rt::EventedItem<#value_ty> =
                    #rt::EventedItem::new(#expr, #updated, #removed, #value_name);
            }
        }
    };
    Ok(expanded)
}

fn parse_args(args: TokenStream, kind: Kind) -> syn::Result<Args> {
    let mut parsed = Args::default();
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("key") && kind == Kind::Map {
            set_once(&meta, &mut parsed.key, parse_key_names(meta.value()?.parse()?)?)
        } else if meta.path.is_ident("value") {
            set_once(&meta, &mut parsed.value, meta.value()?.parse()?)
        } else if meta.path.is_ident("updated") {
            set_once(&meta, &mut parsed.updated, meta.value()?.parse()?)
        } else if meta.path.is_ident("removed") {
            set_once(&meta, &mut parsed.removed, meta.value()?.parse()?)
        } else {
            Err(meta.error(match kind {
                Kind::Map => "unsupported argument, expected `key`, `value`, `updated` or `removed`",
                Kind::Item => "unsupported argument, expected `value`, `updated` or `removed`",
            }))
        }
    });
    parser.parse(args)?;

    let call_site = proc_macro2::Span::call_site();
    if kind == Kind::Map && parsed.key.as_deref().unwrap_or_default().is_empty() {
        return Err(syn::Error::new(call_site, "missing `key = \"...\"` argument"));
    }
    if kind == Kind::Item && parsed.value.is_none() {
        return Err(syn::Error::new(call_site, "missing `value = \"...\"` argument"));
    }

    // Attribute names must be unique and non-empty within an event.
    let names = parsed.key.iter().flatten().chain(parsed.value.iter());
    let mut seen = Vec::new();
    for name in names {
        let value = name.value();
        if value.is_empty() {
            return Err(syn::Error::new(name.span(), "attribute name must not be empty"));
        }
        if seen.contains(&value) {
            return Err(syn::Error::new(name.span(), "duplicate attribute name"));
        }
        seen.push(value);
    }
    for name in parsed.updated.iter().chain(parsed.removed.iter()) {
        if name.value().is_empty() {
            return Err(syn::Error::new(name.span(), "event name must not be empty"));
        }
    }

    Ok(parsed)
}

fn set_once<T>(meta: &ParseNestedMeta, slot: &mut Option<T>, value: T) -> syn::Result<()> {
    if slot.replace(value).is_some() {
        return Err(meta.error("duplicate argument"));
    }
    Ok(())
}

/// Accepts `"id"` or `("role", "address")`.
fn parse_key_names(expr: Expr) -> syn::Result<Vec<LitStr>> {
    let elements: Vec<Expr> = match expr {
        Expr::Tuple(tuple) => tuple.elems.into_iter().collect(),
        Expr::Paren(paren) => vec![*paren.expr],
        other => vec![other],
    };
    elements
        .into_iter()
        .map(|element| match element {
            Expr::Lit(ExprLit {
                lit: Lit::Str(name),
                ..
            }) => Ok(name),
            other => Err(syn::Error::new_spanned(other, "expected a string literal")),
        })
        .collect()
}

/// Extracts the type arguments of `Map<K, V>` / `Item<V>`.
fn type_args<const N: usize>(ty: &Type, expected: &str) -> syn::Result<[Type; N]> {
    let error = || {
        syn::Error::new_spanned(
            ty,
            format!("expected `{expected}<...>` with {N} type argument(s)"),
        )
    };
    let Type::Path(path) = ty else {
        return Err(error());
    };
    let segment = path.path.segments.last().ok_or_else(error)?;
    if segment.ident != expected {
        return Err(error());
    }
    let PathArguments::AngleBracketed(generics) = &segment.arguments else {
        return Err(error());
    };
    let types: Vec<Type> = generics
        .args
        .iter()
        .map(|arg| match arg {
            GenericArgument::Type(ty) => Ok(ty.clone()),
            _ => Err(error()),
        })
        .collect::<syn::Result<_>>()?;
    types.try_into().map_err(|_| error())
}
