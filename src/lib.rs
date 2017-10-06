#![allow(warnings)]
#![feature(plugin_registrar, rustc_private)]

extern crate syntax;
extern crate rustc_plugin;

use syntax::tokenstream::TokenTree;
use syntax::ext::base::{ExtCtxt, MacResult, DummyResult, MacEager};
use syntax::ext::quote::rt::{Span, ExtParseUtils};
use syntax::parse::token::{Token,DelimToken};
use syntax::ext::build::AstBuilder;
use syntax::ast::{TyKind, Mutability, ExprKind, LitKind, ItemKind, Attribute, Name, Item, Visibility, DUMMY_NODE_ID};
use syntax::ptr::P;

use syntax::util::small_vector::SmallVector;
use rustc_plugin::Registry;
use std::rc::Rc;

const UNSUPPORTED_TYPE_MSG: &str = "Only byte strings and array references are supported by `extern_ctypes!`";

fn extern_ctypes(cx: &mut ExtCtxt, sp: Span, args: &[TokenTree]) -> Box<MacResult> {
    let mut parser = cx.new_parser_from_tts(args);

    let ident = match parser.parse_ident() {
        Ok(ident) => ident,
        Err(mut err) => {
            if args.len() == 0 {
                err.set_span(sp);
            }

            err.emit();
            return DummyResult::any(sp);
        },
    };

    if !parser.eat(&Token::Comma) {
        let sp = parser.span;
        cx.span_err(sp, "Expected `,`");
        return DummyResult::any(sp);
    }

    let mut expr = match parser.parse_expr() {
        Ok(expr) => expr,
        Err(mut err) => {
            err.emit();
            return DummyResult::any(sp);
        },
    };
    let expr_sp = parser.span;

    if !parser.check(&Token::Eof) {
        cx.span_err(parser.span, "Expected `)`");
        return DummyResult::any(sp);
    }

    let (expr, len) = match expr.node {
        ExprKind::Lit(ref lit) => match lit.node {
            LitKind::ByteStr(ref str) => {
                let mut str = str.as_ref().to_owned();
                str.push(b'\0');
                let len = str.len();
                let str = LitKind::ByteStr(Rc::new(str));
                let str = cx.expr_lit(sp, str);
                (str, len)
            },
            _ => {
                cx.span_err(expr_sp, UNSUPPORTED_TYPE_MSG);
                return DummyResult::any(sp);
            },
        },
        ExprKind::AddrOf(Mutability::Immutable, ref expr) => match expr.node {
            ExprKind::Array(ref arr) => {
                let mut arr = arr.clone();
                arr.push(cx.expr_u8(sp, 0));
                let len = arr.len();
                let arr = cx.expr(sp, ExprKind::Array(arr));
                let arr = cx.expr(sp, ExprKind::AddrOf(Mutability::Immutable, arr));
                (arr, len)
            },
            _ => {
                cx.span_err(expr_sp, UNSUPPORTED_TYPE_MSG);
                return DummyResult::any(sp);
            }
        },
        ref n => {
            cx.span_err(expr_sp, UNSUPPORTED_TYPE_MSG);
            return DummyResult::any(sp);
        }
    };
    let len = cx.expr_usize(sp, len);

    let ty = cx.ty_ident(sp, cx.ident_of("u8"));
    let ty = cx.ty(sp, TyKind::Array(ty, len));
    let ty = cx.ty_mt(ty, Mutability::Immutable);
    let ty = cx.ty(sp, TyKind::Rptr(None, ty));

    let attrs = vec![
        cx.attribute(sp, cx.meta_word(sp, Name::intern("no_mangle")))
    ];

    let item = P(Item {
        ident: ident,
        attrs: attrs,
        id: DUMMY_NODE_ID,
        node: ItemKind::Static(ty, Mutability::Immutable, expr),
        vis: Visibility::Public,
        span: sp,
        tokens: None,
    });
    let items = SmallVector::one(item);

    MacEager::items(items)
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("extern_ctypes", extern_ctypes);
}
