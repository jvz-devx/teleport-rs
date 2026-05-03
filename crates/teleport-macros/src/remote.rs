use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, ItemFn, Lit, Meta, PatType, Result, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse2,
    punctuated::Punctuated,
    spanned::Spanned,
};

// ---------------------------------------------------------------------------
// Parsed representations
// ---------------------------------------------------------------------------

enum ProcType {
    Query,
    Command,
    Form,
}

struct RemoteAttr {
    proc_type: ProcType,
    name_override: Option<String>,
    prefix_override: Option<String>,
}

struct SigInfo {
    state_ty: Type,
    has_auth: bool,
    has_optional_auth: bool,
    /// Custom auth type when using `#[auth]` with a non-`AuthedUser` type.
    auth_ty: Option<Type>,
    input_ty: Option<Type>,
    output_ty: Type,
    error_ty: Type,
}

// ---------------------------------------------------------------------------
// Attribute parsing
// ---------------------------------------------------------------------------

/// Wrapper for parsing comma-separated metas from an attribute.
struct MetaList(Punctuated<Meta, Token![,]>);

impl Parse for MetaList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

fn parse_attr(attr: TokenStream) -> Result<RemoteAttr> {
    let MetaList(metas) = parse2(attr)?;

    let mut proc_type: Option<ProcType> = None;
    let mut name_override: Option<String> = None;
    let mut prefix_override: Option<String> = None;

    for meta in &metas {
        match meta {
            Meta::Path(path) => {
                if proc_type.is_some() {
                    return Err(Error::new(
                        path.span(),
                        "procedure type already specified (use only one of `query`, `command`, or `form`)",
                    ));
                }
                let ident = path.get_ident().ok_or_else(|| {
                    Error::new(
                        path.span(),
                        "unknown procedure type: expected `query`, `command`, or `form`",
                    )
                })?;
                proc_type = Some(match ident.to_string().as_str() {
                    "query" => ProcType::Query,
                    "command" => ProcType::Command,
                    "form" => ProcType::Form,
                    other => {
                        return Err(Error::new(
                            ident.span(),
                            format!(
                                "unknown procedure type `{other}`: expected `query`, `command`, or `form`"
                            ),
                        ));
                    }
                });
            }
            Meta::NameValue(nv) => {
                let key_ident = nv.path.get_ident().ok_or_else(|| {
                    Error::new(
                        nv.path.span(),
                        "unknown attribute key: expected `name` or `prefix` (e.g. `#[remote(query, name = \"foo\")]`)",
                    )
                })?;
                let key = key_ident.to_string();
                let value = match &nv.value {
                    syn::Expr::Lit(lit) => match &lit.lit {
                        Lit::Str(s) => s.value(),
                        _ => {
                            return Err(Error::new(
                                lit.span(),
                                "expected a string literal (e.g. `name = \"foo\"`)",
                            ));
                        }
                    },
                    other => {
                        return Err(Error::new(
                            other.span(),
                            "expected a string literal (e.g. `name = \"foo\"`)",
                        ));
                    }
                };
                match key.as_str() {
                    "name" => name_override = Some(value),
                    "prefix" => prefix_override = Some(value),
                    other => {
                        return Err(Error::new(
                            nv.path.span(),
                            format!(
                                "unknown attribute key `{other}`: expected `name` or `prefix` (e.g. `#[remote(query, name = \"foo\")]`)"
                            ),
                        ));
                    }
                }
            }
            other @ Meta::List(_) => {
                return Err(Error::new(
                    other.span(),
                    "unexpected attribute argument: expected `query`, `command`, `form`, `name = \"...\"`, or `prefix = \"...\"`",
                ));
            }
        }
    }

    let proc_type = proc_type.ok_or_else(|| {
        Error::new(
            Span::call_site(),
            "#[remote] requires a procedure type. Use one of:\n  \
             \u{2022} #[remote(query)]   \u{2014} GET endpoint, input from query string\n  \
             \u{2022} #[remote(command)] \u{2014} POST endpoint, JSON body\n  \
             \u{2022} #[remote(form)]    \u{2014} POST endpoint, form-urlencoded or JSON body",
        )
    })?;

    Ok(RemoteAttr {
        proc_type,
        name_override,
        prefix_override,
    })
}

// ---------------------------------------------------------------------------
// Signature parsing
// ---------------------------------------------------------------------------

/// Check whether a parameter has the `#[auth]` attribute.
fn has_auth_attr(param: &PatType) -> bool {
    param.attrs.iter().any(|attr| attr.path().is_ident("auth"))
}

/// Check whether a parameter has `#[auth]` wrapping `Option<T>`.
fn has_optional_auth_attr(param: &PatType) -> bool {
    if !has_auth_attr(param) {
        return false;
    }
    let Type::Path(tp) = param.ty.as_ref() else {
        return false;
    };
    tp.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "Option")
}

fn parse_sig(func: &ItemFn) -> Result<SigInfo> {
    let sig = &func.sig;

    if sig.asyncness.is_none() {
        return Err(Error::new(
            sig.fn_token.span,
            "#[remote] functions must be async\n  \
             add `async` before `fn` \u{2014} e.g. `async fn my_proc(...)`",
        ));
    }

    let params: Vec<&PatType> = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Typed(pt) => Ok(pt),
            FnArg::Receiver(r) => Err(Error::new(
                r.span(),
                "#[remote] functions are free functions and cannot take `self`\n  \
                 remove the `self` parameter and take state by reference instead",
            )),
        })
        .collect::<Result<_>>()?;

    if params.is_empty() {
        return Err(Error::new(
            sig.paren_token.span.join(),
            "#[remote] functions must have at least one parameter \u{2014} the state reference\n  \
             e.g. `async fn my_proc(ctx: &AppState, ...)`",
        ));
    }

    // First param must be a reference — extract the inner type.
    let state_ty = match params[0].ty.as_ref() {
        Type::Reference(r) => (*r.elem).clone(),
        other => {
            return Err(Error::new(
                other.span(),
                "first parameter must be a shared reference to the state type\n  \
                 e.g. `async fn my_proc(ctx: &AppState, ...)`\n  \
                 The state parameter is always required and must be the first argument.",
            ));
        }
    };

    let mut has_auth = false;
    let mut has_optional_auth = false;
    let mut auth_ty: Option<Type> = None;
    let mut input_ty: Option<Type> = None;

    for param in params.iter().skip(1) {
        let ty = param.ty.as_ref();
        let is_auth_by_attr = has_auth_attr(param);
        let is_auth_by_name = is_authed_user(ty);
        let is_opt_auth_by_name = is_option_authed_user(ty);

        if is_auth_by_attr || is_auth_by_name || is_opt_auth_by_name {
            if has_auth || has_optional_auth {
                return Err(Error::new(
                    ty.span(),
                    "duplicate auth parameter\n  \
                     a #[remote] function may have at most one auth parameter (marked with `#[auth]` \
                     or typed as `AuthedUser`/`Option<AuthedUser>`)",
                ));
            }
            if is_auth_by_attr && has_optional_auth_attr(param) || is_opt_auth_by_name {
                has_optional_auth = true;
            } else {
                has_auth = true;
            }
            // For #[auth]-attributed params with custom types, store the type.
            if is_auth_by_attr && !is_auth_by_name && !is_opt_auth_by_name {
                auth_ty = Some(ty.clone());
            }
        } else {
            if input_ty.is_some() {
                return Err(Error::new(
                    ty.span(),
                    "only one input parameter is allowed besides state and auth\n  \
                     combine multiple fields into a single `#[teleport_type]` struct and pass it as one argument",
                ));
            }
            input_ty = Some(ty.clone());
        }
    }

    let (output_ty, error_ty) = parse_return_type(&sig.output)?;
    reject_bare_bigint(&output_ty, "output")?;
    reject_bare_bigint(&error_ty, "error detail")?;

    Ok(SigInfo {
        state_ty,
        has_auth,
        has_optional_auth,
        auth_ty,
        input_ty,
        output_ty,
        error_ty,
    })
}

/// Reject bare 64-bit integer primitives as procedure return or error
/// detail types. The `#[teleport_type]` macro can inject
/// `#[serde(with = "…")]` onto struct *fields*, but the procedure return
/// site is not a struct field — `axum::Json::<i64>(v)` would still
/// serialise as a JSON number, producing a runtime mismatch with the
/// TypeScript `string` type. Forcing users to wrap in a struct is
/// consistent with the rest of the framework's struct-centric design.
fn reject_bare_bigint(ty: &Type, position: &str) -> Result<()> {
    let Type::Path(tp) = ty else { return Ok(()) };
    if tp.qself.is_some() || tp.path.segments.len() != 1 {
        return Ok(());
    }
    let ident = tp.path.segments[0].ident.to_string();
    if matches!(
        ident.as_str(),
        "i64" | "u64" | "i128" | "u128" | "isize" | "usize"
    ) {
        return Err(Error::new(
            ty.span(),
            format!(
                "`{ident}` is not supported as a bare {position} type\n  \
                 JavaScript's `number` loses precision above 2^53, so teleport-rs\n  \
                 serialises 64-bit integers as JSON strings. The `#[teleport_type]`\n  \
                 macro handles this automatically for struct *fields*, but bare\n  \
                 primitive returns would produce a runtime type mismatch.\n  \
                 \n  \
                 Wrap the value in a struct:\n  \
                 \n  \
                      #[teleport_type]\n  \
                      pub struct {ident}Result {{\n  \
                          pub value: {ident},\n  \
                      }}\n  \
                 \n  \
                 Or return a plain `String` / `i32` if you don't need 64-bit precision.",
            ),
        ));
    }
    Ok(())
}

fn is_authed_user(ty: &Type) -> bool {
    last_segment_is(ty, "AuthedUser")
}

fn is_option_authed_user(ty: &Type) -> bool {
    let Type::Path(tp) = ty else { return false };
    let Some(seg) = tp.path.segments.last() else {
        return false;
    };
    if seg.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    args.args.len() == 1
        && args.args.first().is_some_and(
            |a| matches!(a, syn::GenericArgument::Type(inner) if is_authed_user(inner)),
        )
}

fn last_segment_is(ty: &Type, name: &str) -> bool {
    let Type::Path(tp) = ty else { return false };
    tp.path.segments.last().is_some_and(|seg| seg.ident == name)
}

/// Parse `-> Result<T, AppError<E>>` and extract `(T, E)`.
fn parse_return_type(ret: &ReturnType) -> Result<(Type, Type)> {
    const RETURN_TYPE_HELP: &str = "return type must be `Result<T, AppError<E>>`\n  \
        where T is the success value and E is the typed error detail.\n  \
        Both T and E should implement `#[teleport_type]`.";

    let ReturnType::Type(_, ty) = ret else {
        return Err(Error::new(
            ret.span(),
            "#[remote] functions must return a value\n  \
             return type must be `Result<T, AppError<E>>`\n  \
             where T is the success value and E is the typed error detail.\n  \
             Both T and E should implement `#[teleport_type]`.",
        ));
    };

    let Type::Path(tp) = ty.as_ref() else {
        return Err(Error::new(ty.span(), RETURN_TYPE_HELP));
    };

    let seg = tp
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new(tp.span(), RETURN_TYPE_HELP))?;

    if seg.ident != "Result" {
        return Err(Error::new(seg.ident.span(), RETURN_TYPE_HELP));
    }

    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return Err(Error::new(seg.span(), RETURN_TYPE_HELP));
    };

    let mut iter = args.args.iter();
    let output_arg = iter
        .next()
        .ok_or_else(|| Error::new(args.span(), RETURN_TYPE_HELP))?;
    let error_arg = iter
        .next()
        .ok_or_else(|| Error::new(args.span(), RETURN_TYPE_HELP))?;

    let syn::GenericArgument::Type(output_ty) = output_arg else {
        return Err(Error::new(
            output_arg.span(),
            "expected a type as the success value of `Result<T, AppError<E>>`",
        ));
    };
    let syn::GenericArgument::Type(error_wrapper) = error_arg else {
        return Err(Error::new(
            error_arg.span(),
            "expected `AppError<E>` as the error type of `Result<T, AppError<E>>`",
        ));
    };

    let error_ty = extract_app_error_inner(error_wrapper)?;
    Ok((output_ty.clone(), error_ty))
}

/// Given `AppError<E>`, return `E`. If just `AppError`, return `()`.
fn extract_app_error_inner(ty: &Type) -> Result<Type> {
    const APP_ERROR_HELP: &str = "error type must be `AppError` or `AppError<E>`\n  \
        use `AppError<E>` to return a typed error payload where E implements `#[teleport_type]`,\n  \
        or `AppError` for an untyped error.";

    let Type::Path(tp) = ty else {
        return Err(Error::new(ty.span(), APP_ERROR_HELP));
    };

    let seg = tp
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new(tp.span(), APP_ERROR_HELP))?;

    if seg.ident != "AppError" {
        return Err(Error::new(seg.ident.span(), APP_ERROR_HELP));
    }

    match &seg.arguments {
        syn::PathArguments::None => Ok(syn::parse_quote!(())),
        syn::PathArguments::AngleBracketed(args) => {
            let arg = args.args.first().ok_or_else(|| {
                Error::new(
                    args.span(),
                    "expected a type inside `AppError<...>` (e.g. `AppError<MyError>`)",
                )
            })?;
            let syn::GenericArgument::Type(inner) = arg else {
                return Err(Error::new(
                    arg.span(),
                    "expected a type inside `AppError<...>` (e.g. `AppError<MyError>`)",
                ));
            };
            Ok(inner.clone())
        }
        syn::PathArguments::Parenthesized(_) => Err(Error::new(
            seg.arguments.span(),
            "unexpected parenthesized arguments on `AppError` \u{2014} use `AppError<E>` with angle brackets",
        )),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn extract_doc(func: &ItemFn) -> String {
    let mut doc = String::new();
    for attr in &func.attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(lit) = &nv.value
            && let Lit::Str(s) = &lit.lit
        {
            let line = s.value();
            if !doc.is_empty() {
                doc.push('\n');
            }
            doc.push_str(line.trim());
        }
    }
    doc
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

/// Expand the `#[remote(query|command|form)]` attribute macro.
///
/// # Attribute keys
///
/// In addition to the procedure type (`query`, `command`, or `form`), the
/// attribute accepts two optional `key = "value"` overrides:
///
/// - `prefix = "..."` — overrides the default namespace (derived from
///   `module_path!()`) with an explicit string. Use this to get clean
///   namespaces like `users.getUser` in single-file apps where the default
///   would otherwise be `{crate_name}.get_user`.
/// - `name = "..."` — overrides the default procedure name (derived from
///   the Rust function name, converted to `camelCase`) with an explicit
///   string. Combined with `prefix`, this gives full control over the
///   generated TypeScript identifier.
///
/// ```ignore
/// // Default: namespace comes from module_path!(), name from fn ident.
/// // e.g. in `src/main.rs`, this becomes `my_app.getUser`.
/// #[remote(query)]
/// async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { ... }
///
/// // Overridden: forces `users.getUser` regardless of module path.
/// #[remote(query, prefix = "users", name = "getUser")]
/// async fn get_user(ctx: &AppState, input: GetUserInput) -> Result<User, AppError> { ... }
/// ```
pub fn expand(attr: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let remote_attr = parse_attr(attr)?;
    let func: ItemFn = parse2(item)?;
    let sig_info = parse_sig(&func)?;

    let fn_name = &func.sig.ident;
    let handler_name = format_ident!("__teleport_handler_{fn_name}");
    let reg_const_name = format_ident!("__TELEPORT_REG_{}", fn_name.to_string().to_uppercase());

    let camel_name = remote_attr
        .name_override
        .as_deref()
        .map_or_else(|| to_camel_case(&fn_name.to_string()), str::to_owned);

    let doc = extract_doc(&func);
    let handler_fn = gen_handler(&func, &handler_name, &remote_attr, &sig_info);
    let registration = gen_registration(
        &reg_const_name,
        &handler_name,
        &camel_name,
        &doc,
        &remote_attr,
        &sig_info,
    );

    // Strip #[auth] attributes from the original function so the compiler
    // doesn't reject them as unknown attributes.
    let mut cleaned_func = func.clone();
    for arg in &mut cleaned_func.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            pt.attrs.retain(|attr| !attr.path().is_ident("auth"));
        }
    }

    Ok(quote! {
        #cleaned_func
        #handler_fn
        #registration
    })
}

fn gen_handler(
    func: &ItemFn,
    handler_name: &Ident,
    remote_attr: &RemoteAttr,
    sig: &SigInfo,
) -> TokenStream {
    let fn_name = &func.sig.ident;
    let fn_vis = &func.vis;
    let state_ty = &sig.state_ty;
    let output_ty = &sig.output_ty;
    let error_ty = &sig.error_ty;

    let state_extract = quote! {
        axum::extract::State(state): axum::extract::State<std::sync::Arc<#state_ty>>
    };

    let auth_extract = if sig.has_auth {
        let ty = sig
            .auth_ty
            .as_ref()
            .map_or_else(|| quote! { teleport::AuthedUser }, |t| quote! { #t });
        Some(quote! { auth: #ty })
    } else if sig.has_optional_auth {
        // custom_ty already includes Option<T> from the parameter
        let ty = sig.auth_ty.as_ref().map_or_else(
            || quote! { Option<teleport::AuthedUser> },
            |t| quote! { #t },
        );
        Some(quote! { auth: #ty })
    } else {
        None
    };

    let input_extract = sig
        .input_ty
        .as_ref()
        .map(|input| match remote_attr.proc_type {
            ProcType::Query => quote! {
                teleport::QsQuery(input): teleport::QsQuery<#input>
            },
            ProcType::Command => quote! {
                axum::Json(input): axum::Json<#input>
            },
            ProcType::Form => quote! {
                teleport::FormOrJson(input): teleport::FormOrJson<#input>
            },
        });

    let mut handler_params = vec![state_extract];
    if let Some(ref auth) = auth_extract {
        handler_params.push(auth.clone());
    }
    if let Some(ref inp) = input_extract {
        handler_params.push(inp.clone());
    }

    let mut call_args = vec![quote!(&*state)];
    if sig.has_auth || sig.has_optional_auth {
        call_args.push(quote!(auth));
    }
    if sig.input_ty.is_some() {
        call_args.push(quote!(input));
    }

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::needless_pass_by_value)]
        #fn_vis async fn #handler_name(
            #(#handler_params),*
        ) -> Result<axum::Json<#output_ty>, teleport::AppError<#error_ty>> {
            let result = #fn_name(#(#call_args),*).await?;
            Ok(axum::Json(result))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn gen_registration(
    reg_const_name: &Ident,
    handler_name: &Ident,
    camel_name: &str,
    doc: &str,
    remote_attr: &RemoteAttr,
    sig: &SigInfo,
) -> TokenStream {
    let state_ty = &sig.state_ty;
    let output_ty = &sig.output_ty;
    let error_ty = &sig.error_ty;

    let (proc_type_token, http_method_token) = match remote_attr.proc_type {
        ProcType::Query => (
            quote!(teleport::private::ProcedureType::Query),
            quote!(teleport::private::HttpMethod::Get),
        ),
        ProcType::Command => (
            quote!(teleport::private::ProcedureType::Command),
            quote!(teleport::private::HttpMethod::Post),
        ),
        ProcType::Form => (
            quote!(teleport::private::ProcedureType::Form),
            quote!(teleport::private::HttpMethod::Post),
        ),
    };

    let input_type_fn = sig.input_ty.as_ref().map_or_else(
        || quote! { |types: &mut specta::Types| <() as specta::Type>::definition(types) },
        |input| quote! { |types: &mut specta::Types| <#input as specta::Type>::definition(types) },
    );
    let output_type_fn =
        quote! { |types: &mut specta::Types| <#output_ty as specta::Type>::definition(types) };
    let error_type_fn =
        quote! { |types: &mut specta::Types| <#error_ty as specta::Type>::definition(types) };

    let prefix_token = remote_attr
        .prefix_override
        .as_ref()
        .map_or_else(|| quote!(None), |p| quote!(Some(#p)));

    let auth_mode_token = if sig.has_auth {
        quote!(teleport::private::AuthMode::Required)
    } else if sig.has_optional_auth {
        quote!(teleport::private::AuthMode::Optional)
    } else {
        quote!(teleport::private::AuthMode::None)
    };

    let route_method = match remote_attr.proc_type {
        ProcType::Query => quote!(axum::routing::get),
        ProcType::Command | ProcType::Form => quote!(axum::routing::post),
    };

    let mount_fn = quote! {
        || -> Box<dyn std::any::Any + Send> {
            let method_router: axum::routing::MethodRouter<std::sync::Arc<#state_ty>> =
                #route_method(#handler_name);
            Box::new(method_router)
        }
    };

    quote! {
        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        const #reg_const_name: () = {
            teleport::private::inventory::submit! {
                teleport::private::ProcedureRegistration {
                    module_path: module_path!(),
                    fn_name: #camel_name,
                    prefix: #prefix_token,
                    method: #http_method_token,
                    procedure_type: #proc_type_token,
                    input_type: #input_type_fn,
                    output_type: #output_type_fn,
                    error_type: #error_type_fn,
                    doc: #doc,
                    auth_mode: #auth_mode_token,
                    mount_fn: #mount_fn,
                }
            }
        };
    }
}
