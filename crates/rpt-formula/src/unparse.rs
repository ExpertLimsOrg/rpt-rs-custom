//! Print a parsed formula AST back to Crystal source (for safe recombination, not round-trip parity).

use crate::ast::{ExitKind, Node, NodeKind, VarKind, VarScope};
use crate::token::{op, RefKind};

/// Minimum parent precedence (higher binds tighter). `0` = no parent constraint.
pub(crate) fn unparse(node: &Node, parent_prec: u8) -> String {
    let s = unparse_node(node);
    let p = node_precedence(node);
    if p > 0 && p < parent_prec {
        format!("({s})")
    } else {
        s
    }
}

fn node_precedence(node: &Node) -> u8 {
    match &node.kind {
        NodeKind::Binary { op, .. } => binary_prec(*op),
        _ => 255,
    }
}

/// Left-associative binary operators at this precedence level (see [`crate::parser`].
fn binary_prec(op: u8) -> u8 {
    match op {
        op::IMP => 10,
        op::EQV => 20,
        op::XOR => 30,
        op::OR => 40,
        op::AND => 50,
        op::EQ | op::NE | op::IN | op::LIKE | op::STARTS_WITH => 60,
        op::LT | op::GT | op::GE | op::LE => 70,
        op::AMP => 80,
        op::RANGE_TO
        | op::RANGE_LO_EXCL
        | op::RANGE_HI_EXCL
        | op::RANGE_BOTH_EXCL => 90,
        op::PLUS | op::MINUS => 100,
        op::MOD => 105,
        op::BACKSLASH => 106,
        op::STAR | op::SLASH => 110,
        op::CARET => 120,
        _ => 0,
    }
}

fn unparse_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Number(s) | NodeKind::DateLit(s) => s.clone(),
        NodeKind::Str(s) => format!("\"{}\"", escape_string(s)),
        NodeKind::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        NodeKind::Reference { kind, name } => format_reference(*kind, name),
        NodeKind::Ident(s) => s.clone(),
        NodeKind::Call { name, args } => {
            let args: Vec<_> = args.iter().map(|a| unparse(a, 0)).collect();
            format!("{}({})", name, args.join(", "))
        }
        NodeKind::Index { base, index } => {
            format!("{}[{}]", unparse(base, 0), unparse(index, 0))
        }
        NodeKind::Unary { op: code, expr } => {
            let inner = unparse(expr, binary_prec(*code) + 1);
            match *code {
                op::NOT => format!("Not {}", inner),
                op::UNARY_MINUS => format!("-{}", inner),
                op::UNARY_PLUS => format!("+{}", inner),
                _ => format!("{} {}", op::symbol(*code), inner),
            }
        }
        NodeKind::Binary { op: code, left, right } => {
            let p = binary_prec(*code);
            let l = unparse(left, p + 1);
            let r = unparse(right, p + 1);
            let sym = binary_symbol(*code);
            format!("{l} {sym} {r}")
        }
        NodeKind::Array(items) => {
            let inner: Vec<_> = items.iter().map(|n| unparse(n, 0)).collect();
            format!("[{}]", inner.join(", "))
        }
        NodeKind::If {
            cond,
            then,
            elifs,
            els,
        } => {
            let mut out = format!(
                "If {} Then {}",
                unparse(cond, 0),
                unparse(then, 0)
            );
            for (c, t) in elifs {
                out.push_str(&format!(" Else If {} Then {}", unparse(c, 0), unparse(t, 0)));
            }
            if let Some(e) = els {
                out.push_str(&format!(" Else {}", unparse(e, 0)));
            }
            out
        }
        NodeKind::Assign { name, value } => {
            format!("{} := {}", name, unparse(value, 0))
        }
        NodeKind::AssignIndex { name, index, value } => {
            format!(
                "{}[{}] := {}",
                name,
                unparse(index, 0),
                unparse(value, 0)
            )
        }
        NodeKind::Declare {
            scope,
            kind,
            array,
            names,
            init,
        } => {
            let scope = match scope {
                VarScope::Local => "Local ",
                VarScope::Global => "Global ",
                VarScope::Shared => "Shared ",
            };
            let kind = var_kind_name(*kind);
            let array = if *array { " Array" } else { "" };
            let mut out = format!("{scope}{kind}Var{array} {}", names.join(", "));
            if let Some(i) = init {
                out.push_str(&format!(" := {}", unparse(i, 0)));
            }
            out
        }
        NodeKind::While { cond, body, test_after } => {
            if *test_after {
                format!(
                    "Do {} Loop While {}",
                    unparse(body, 0),
                    unparse(cond, 0)
                )
            } else {
                format!(
                    "While {} Do {}",
                    unparse(cond, 0),
                    unparse(body, 0)
                )
            }
        }
        NodeKind::For {
            var,
            from,
            to,
            step,
            body,
        } => {
            let mut out = format!(
                "For {} := {} To {}",
                var,
                unparse(from, 0),
                unparse(to, 0)
            );
            if let Some(s) = step {
                out.push_str(&format!(" Step {}", unparse(s, 0)));
            }
            out.push_str(&format!(" {}", unparse(body, 0)));
            out
        }
        NodeKind::Exit(kind) => match kind {
            ExitKind::For => "Exit For".into(),
            ExitKind::While => "Exit While".into(),
            ExitKind::Do => "Exit Do".into(),
        },
        NodeKind::Seq(items) => items
            .iter()
            .map(|n| unparse(n, 0))
            .collect::<Vec<_>>()
            .join("; "),
        NodeKind::Empty => String::new(),
        NodeKind::Error | NodeKind::Unparsed(_) => String::new(),
    }
}

fn binary_symbol(code: u8) -> &'static str {
    match code {
        op::IN => "In",
        op::LIKE => "Like",
        op::STARTS_WITH => "StartsWith",
        _ => op::symbol(code),
    }
}

fn format_reference(kind: RefKind, name: &str) -> String {
    let inner = match kind {
        RefKind::Field => name.to_string(),
        RefKind::Parameter => format!("?{name}"),
        RefKind::Formula => format!("@{name}"),
        RefKind::RunningTotal => format!("#{name}"),
        RefKind::SqlExpr => format!("%{name}"),
    };
    format!("{{{inner}}}")
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn var_kind_name(kind: VarKind) -> &'static str {
    match kind {
        VarKind::Number => "Number",
        VarKind::Currency => "Currency",
        VarKind::Boolean => "Boolean",
        VarKind::Date => "Date",
        VarKind::Time => "Time",
        VarKind::DateTime => "DateTime",
        VarKind::String => "String",
    }
}
