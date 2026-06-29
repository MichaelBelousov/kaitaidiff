//! Gate: proves the generated kaitai code compiles against the vendored runtime
//! and that the hand-written walker (the shape the codegen will emit) produces a
//! correct `Node` tree. Not part of the shipped product.
#![allow(non_snake_case)]

use kaitai::*;
use kdiff_abi::{Field, Node, ParseResult};

#[path = "demo.rs"]
mod demo;
use demo::*;

fn walk_Demo(o: &Demo) -> Node {
    let mut f = Vec::new();
    f.push(Field::new("magic", Node::Bytes(o.magic().clone())));
    f.push(Field::new("version", Node::uint(*o.version())));
    f.push(Field::new(
        "kind",
        Node::enom("Demo_Color", format!("{:?}", *o.kind()), i64::from(&*o.kind())),
    ));
    f.push(Field::new("num_items", Node::uint(*o.num_items())));
    {
        let items = o.items();
        let mut v = Vec::new();
        for it in items.iter() {
            v.push(walk_Demo_Item(it));
        }
        f.push(Field::new("items", Node::List { ty: "Demo_Item".into(), items: v }));
    }
    f.push(Field::new("trailer", Node::uint(*o.trailer())));
    Node::Struct { ty: "Demo".into(), fields: f }
}

fn walk_Demo_Item(o: &Demo_Item) -> Node {
    let mut f = Vec::new();
    f.push(Field::new("tag", Node::uint(*o.tag())));
    let body = match &*o.body() {
        Some(Demo_Item_Body::Demo_TextBody(x)) => walk_Demo_TextBody(x),
        Some(Demo_Item_Body::Demo_NumBody(x)) => walk_Demo_NumBody(x),
        None => Node::Null,
    };
    f.push(Field::new("body", body));
    Node::Struct { ty: "Demo_Item".into(), fields: f }
}

fn walk_Demo_TextBody(o: &Demo_TextBody) -> Node {
    let mut f = Vec::new();
    f.push(Field::new("len", Node::uint(*o.len())));
    f.push(Field::new("value", Node::Str(o.value().clone())));
    Node::Struct { ty: "Demo_TextBody".into(), fields: f }
}

fn walk_Demo_NumBody(o: &Demo_NumBody) -> Node {
    let mut f = Vec::new();
    f.push(Field::new("value", Node::uint(*o.value())));
    Node::Struct { ty: "Demo_NumBody".into(), fields: f }
}

fn parse(bytes: Vec<u8>) -> ParseResult {
    let reader = BytesReader::from(bytes);
    match Demo::read_into::<BytesReader, Demo>(&reader, None, None) {
        Ok(d) => ParseResult::Ok(walk_Demo(&d)),
        Err(e) => ParseResult::Err(format!("{e:?}")),
    }
}

fn main() {
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"DEMO");
    b.extend_from_slice(&2u16.to_be_bytes()); // version
    b.push(1); // kind = green
    b.extend_from_slice(&1u32.to_be_bytes()); // num_items
    b.push(1); // item tag=1 -> num_body
    b.extend_from_slice(&42u32.to_be_bytes());
    b.push(7); // trailer (version>1)

    let result = parse(b);
    let json = result.to_json_bytes();
    println!("{}", String::from_utf8_lossy(&json));
    if let ParseResult::Ok(node) = &result {
        println!("content_hash = {:016x}", node.content_hash());
    }
}
