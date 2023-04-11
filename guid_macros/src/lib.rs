// Taken from

use proc_macro::TokenStream;

use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Error, LitStr};

macro_rules! err {
    ($span:expr, $message:expr $(,)?) => {
        Error::new($span.span(), $message).to_compile_error()
    };
    ($span:expr, $message:expr, $($args:expr),*) => {
        Error::new($span.span(), format!($message, $($args),*)).to_compile_error()
    };
}

/// Create a `Guid` at compile time.
///
/// # Example
///
/// ```
/// use uefi::{guid, Guid};
/// const EXAMPLE_GUID: Guid = guid!("12345678-9abc-def0-1234-56789abcdef0");
/// ```
#[proc_macro]
pub fn guid(args: TokenStream) -> TokenStream {
    let (time_low, time_mid, time_high_and_version, clock_seq_and_variant, node) =
        match parse_guid(parse_macro_input!(args as LitStr)) {
            Ok(data) => data,
            Err(tokens) => return tokens.into(),
        };

    quote!({
        const g: crate::guid::Guid = crate::guid::Guid::from_values(
            #time_low,
            #time_mid,
            #time_high_and_version,
            #clock_seq_and_variant,
            #node,
        );
        g
    })
    .into()
}

fn parse_guid(guid_lit: LitStr) -> Result<(u32, u16, u16, u16, u64), TokenStream2> {
    let guid_str = guid_lit.value();

    // We expect a canonical GUID string, such as "12345678-9abc-def0-fedc-ba9876543210"
    if guid_str.len() != 36 {
        return Err(err!(
            guid_lit,
            "\"{}\" is not a canonical GUID string (expected 36 bytes, found {})",
            guid_str,
            guid_str.len()
        ));
    }
    let mut offset = 1; // 1 is for the starting quote
    let mut guid_hex_iter = guid_str.split('-');
    let mut next_guid_int = |len: usize| -> Result<u64, TokenStream2> {
        let guid_hex_component = guid_hex_iter.next().unwrap();

        // convert syn::LitStr to proc_macro2::Literal..
        let lit = match guid_lit.to_token_stream().into_iter().next().unwrap() {
            TokenTree::Literal(lit) => lit,
            _ => unreachable!(),
        };
        // ..so that we can call subspan and nightly users (us) will get the fancy span
        let span = lit
            .subspan(offset..offset + guid_hex_component.len())
            .unwrap_or_else(|| lit.span());

        if guid_hex_component.len() != len * 2 {
            return Err(err!(
                span,
                "GUID component \"{}\" is not a {}-bit hexadecimal string",
                guid_hex_component,
                len * 8
            ));
        }
        offset += guid_hex_component.len() + 1; // + 1 for the dash
        u64::from_str_radix(guid_hex_component, 16).map_err(|_| {
            err!(
                span,
                "GUID component \"{}\" is not a hexadecimal number",
                guid_hex_component
            )
        })
    };

    // The GUID string is composed of a 32-bit integer, three 16-bit ones, and a 48-bit one
    Ok((
        next_guid_int(4)? as u32,
        next_guid_int(2)? as u16,
        next_guid_int(2)? as u16,
        next_guid_int(2)? as u16,
        next_guid_int(6)?,
    ))
}
