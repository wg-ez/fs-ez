//! Pretty-printer : forme canonique stable et re-lisible par `parse`.

use fs_ez_core::FilterExpr;

const OR: u8 = 1;
const AND: u8 = 2;
const UNARY: u8 = 3;

/// Caractères qui obligent à citer un nom pour rester re-lisible.
const NEEDS_QUOTES: &[char] = &[
    ' ', '\t', '\n', '\r', '(', ')', '=', '!', '<', '>', '"', '\\', '&', '|',
];

pub fn pretty(expr: &FilterExpr) -> String {
    let mut out = String::new();
    write(expr, OR, &mut out);
    out
}

fn write(expr: &FilterExpr, context: u8, out: &mut String) {
    let own = match expr {
        FilterExpr::Or { .. } => OR,
        FilterExpr::And { .. } => AND,
        FilterExpr::Tag { .. }
        | FilterExpr::OnlyTag { .. }
        | FilterExpr::TagSet { .. }
        | FilterExpr::Not { .. } => UNARY,
    };
    let parenthesize = own < context;
    if parenthesize {
        out.push('(');
    }
    match expr {
        FilterExpr::Tag { tag, negated } => {
            if *negated {
                out.push_str("not ");
            }
            out.push_str(&quote_name(tag));
        }
        FilterExpr::OnlyTag { tag, negated } => {
            if *negated {
                out.push_str("not ");
            }
            out.push_str("only ");
            out.push_str(&quote_name(tag));
        }
        FilterExpr::TagSet { tags, negated } => {
            if *negated {
                out.push_str("not ");
            }
            out.push_str("exactly ");
            if tags.len() == 1 {
                out.push_str(&quote_name(&tags[0]));
            } else {
                out.push('(');
                for (index, tag) in tags.iter().enumerate() {
                    if index > 0 {
                        out.push_str(" and ");
                    }
                    out.push_str(&quote_name(tag));
                }
                out.push(')');
            }
        }
        FilterExpr::Not { item } => {
            out.push_str("not ");
            write(item, UNARY, out);
        }
        FilterExpr::And { items } => write_list(items, " and ", AND, out),
        FilterExpr::Or { items } => write_list(items, " or ", OR, out),
    }
    if parenthesize {
        out.push(')');
    }
}

fn write_list(items: &[FilterExpr], separator: &str, context: u8, out: &mut String) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(separator);
        }
        write(item, context, out);
    }
}

fn quote_name(name: &str) -> String {
    if !name.is_empty() && !name.chars().any(|c| NEEDS_QUOTES.contains(&c)) {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for c in name.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
