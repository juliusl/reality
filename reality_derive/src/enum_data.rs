use std::collections::BTreeSet;
use logos::Lexer;
use logos::Logos;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use syn::LitStr;

/// Parses an interpolation expression and generates a struct for assignment tokens,
/// 
pub(crate) struct InterpolationExpr {
    pub(crate) name: Ident,
    pub(crate) expr: LitStr,
}

impl InterpolationExpr {
    pub(crate) fn signature_struct(&self) -> TokenStream {
        let name = &self.name;
        let expr = self.expr.value();

        let lexer = StringInterpolationTokens::lexer(&expr);
        let fields = lexer.into_iter().filter_map(|t| {
            match t {
                StringInterpolationTokens::Assignment(a) => {
                    let ident = format_ident!("{}", a.to_lowercase());
                    Some(quote! { 
                        #ident: String
                    })
                },
                StringInterpolationTokens::OptionalSuffixAssignment(a) => {
                    let ident = format_ident!("{}", a.to_lowercase());
                    Some(quote! { 
                        #ident: Option<String>
                    })
                },
                _ => {
                    None
                }
            }
        });

        let fields = quote! {
            #( #fields ),*
        };

        quote_spanned! {self.expr.span()=>
            #name {
                #fields
            }
        }
    }

    pub(crate) fn impl_expr(&self, enum_ident: Ident) -> TokenStream {
        let name = &self.name;
        let expr = self.expr.value();

        let lexer = StringInterpolationTokens::lexer(&expr);
        let fields = lexer.into_iter().filter_map(|t| {
            match t {
                StringInterpolationTokens::Assignment(a) => {
                    let ident = format_ident!("{}", a.to_lowercase());
                    let ident_lit_str = LitStr::new(&ident.to_string(), Span::call_site());
                    Some(quote_spanned! {self.expr.span()=> 
                        #ident: map[#ident_lit_str].to_string()
                    })
                },
                StringInterpolationTokens::OptionalSuffixAssignment(a) => {
                    let ident = format_ident!("{}", a.to_lowercase());
                    let ident_lit_str = LitStr::new(&ident.to_string(), Span::call_site());
                    Some(quote! { 
                        #ident: map.get(#ident_lit_str).cloned()
                    })
                },
                _ => {
                    None
                }
            }
        });

        quote! {
            if let Some(map) = ident.interpolate(#expr) {
                let signature = #enum_ident::#name {
                    #( #fields ),*
                };
                matches.push(signature);
            }
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use syn::parse2;

    use super::InterpolationExpr;

    #[test]
    fn test_interpolation_expr() {

    }
}

/// TODO: Move this to a library, copied from ::v2
/// 
/// Tokens to parse an identifier and interpolate values,
///
/// Example,
///
/// Given an identifier, "blocks.test.object."#tagA:tagB#" and a pattern "blocks.(name).(symbol).#tagA#",
///
/// String interpolation would return a mapping such as,
///
/// name = "test"
/// symbol = "object"
///
/// Given a pattern "blocks.(name).(symbol).#tagC#",
///
/// String interpolation would return an empty result
///
#[derive(Logos, Debug, Clone)]
enum StringInterpolationTokens {
    /// Match this token, escaped w/ quotes,
    ///
    #[regex(r#"[.]?["][^"]*["][.]?"#, on_match)]
    EscapedMatch(String),
    /// Match this token,
    ///
    #[regex("[.]?[a-zA-Z0-9:]+[.]?", on_match)]
    Match(String),
    /// Match tags,
    ///
    #[regex("[.]?[#][a-zA-Z0-9:]*[#][.]?", on_match_tags)]
    MatchTags(BTreeSet<String>),
    /// Not match tags,
    /// 
    #[regex("[.]?[!][#][a-zA-Z0-9:]*[#][.]?", on_match_tags)]
    NotMatchTags(BTreeSet<String>),
    /// Assign the value from the identifier,
    ///
    #[regex("[(][^()]+[)]", on_assignment)]
    Assignment(String),
    /// Breaks if encountered,
    /// 
    #[token(";")]
    Break,
    /// Optionally assign a suffix,
    ///
    #[regex("[(][?][^()]+[)]", on_optional_suffix_assignment)]
    OptionalSuffixAssignment(String),
    #[error]
    #[regex("[.]", logos::skip)]
    Error,
}

fn on_match(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let start = if lex.slice().chars().nth(0) == Some('.') {
        1
    } else {
        0
    };

    let end = if lex.slice().chars().last() == Some('.') {
        lex.slice().len() - 1
    } else {
        lex.slice().len()
    };

    lex.slice()[start..end].to_string()
}

fn on_match_tags(lex: &mut Lexer<StringInterpolationTokens>) -> BTreeSet<String> {
    let start = if lex.slice().chars().nth(0) == Some('.') {
        1
    } else {
        0
    };

    let end = if lex.slice().chars().last() == Some('.') {
        lex.slice().len() - 1
    } else {
        lex.slice().len()
    };

    let mut tags = BTreeSet::new();
    for tag in lex.slice()[start..end].trim_start_matches("!").trim_matches('#').split(":") {
        tags.insert(tag.to_string());
    }

    tags
}

fn on_assignment(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let name = lex.slice()[1..lex.slice().len() - 1].to_string();
    name
}

fn on_optional_suffix_assignment(lex: &mut Lexer<StringInterpolationTokens>) -> String {
    let name = lex.slice()[2..lex.slice().len() - 1].to_string();
    name
}
