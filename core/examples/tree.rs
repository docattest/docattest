//! Development tool: one-line mdast for each string in a JSON array argument.
fn show(n: &markdown::mdast::Node) -> String {
    let d = format!("{n:?}");
    let name = d.split(['(', ' ', '{']).next().unwrap_or("");
    let mut c = name.chars();
    let name: String =
        c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default();
    match n.children() {
        Some(ch) => format!("{name}({})", ch.iter().map(show).collect::<Vec<_>>().join(",")),
        None => match n {
            markdown::mdast::Node::Text(t) => format!("{name}{}", serde_json::to_string(&t.value).unwrap()),
            markdown::mdast::Node::Code(t) => format!("{name}{}", serde_json::to_string(&t.value).unwrap()),
            markdown::mdast::Node::InlineCode(t) => {
                format!("{name}{}", serde_json::to_string(&t.value).unwrap())
            }
            markdown::mdast::Node::Html(t) => format!("{name}{}", serde_json::to_string(&t.value).unwrap()),
            _ => name,
        },
    }
}
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let list: Vec<String> = serde_json::from_str(&arg).unwrap();
    for s in list {
        let t = markdown::to_mdast(&s, &markdown::ParseOptions::gfm()).unwrap();
        println!("{}", show(&t));
    }
}
