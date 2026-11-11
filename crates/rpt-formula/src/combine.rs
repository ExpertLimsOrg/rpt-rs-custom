//! Combine record-selection formulas without string-concatenation traps.

use crate::ast::{Node, NodeKind};
use crate::parser::{self, Severity};
use crate::token::{op, Syntax};
use crate::unparse;

/// A formula body failed to parse cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaParseError(pub String);

/// Parse a Crystal formula and ensure there are no syntactic errors in the AST.
pub fn parse_checked(src: &str, label: &str) -> Result<Node, FormulaParseError> {
    let src = src.trim();
    if src.is_empty() {
        return Err(FormulaParseError(format!("{label}: the formula is empty")));
    }
    let (node, diagnostics) = parser::parse(src, Syntax::Crystal);
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        let msg = diagnostics
            .first()
            .map(|d| d.message.clone())
            .unwrap_or_else(|| "syntax error".into());
        return Err(FormulaParseError(format!("{label}: {msg}")));
    }
    if ast_has_error(&node) {
        return Err(FormulaParseError(format!("{label}: incomplete parse")));
    }
    Ok(node)
}

/// `left And right` as Crystal source, with both operands parsed and re-printed from the AST.
pub fn and_crystal(left: &str, right: &str) -> Result<String, FormulaParseError> {
    let left = parse_checked(left, "stored selection")?;
    let right = parse_checked(right, "selection")?;
    let span = left.span.to(right.span);
    let combined = Node::new(
        NodeKind::Binary {
            op: op::AND,
            left: Box::new(left),
            right: Box::new(right),
        },
        span,
    );
    Ok(unparse::unparse(&combined, 0))
}

fn ast_has_error(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Error => true,
        NodeKind::Unparsed(children) => children.iter().any(ast_has_error),
        NodeKind::Binary { left, right, .. } => ast_has_error(left) || ast_has_error(right),
        NodeKind::Unary { expr, .. } => ast_has_error(expr),
        NodeKind::Call { args, .. } => args.iter().any(ast_has_error),
        NodeKind::Index { base, index, .. } => ast_has_error(base) || ast_has_error(index),
        NodeKind::If {
            cond,
            then,
            elifs,
            els,
        } => {
            ast_has_error(cond)
                || ast_has_error(then)
                || elifs.iter().any(|(a, b)| ast_has_error(a) || ast_has_error(b))
                || els.as_ref().is_some_and(|e| ast_has_error(e))
        }
        NodeKind::Assign { value, .. } => ast_has_error(value),
        NodeKind::Declare { init, .. } => init.as_ref().is_some_and(|i| ast_has_error(i)),
        NodeKind::While { cond, body, .. } => ast_has_error(cond) || ast_has_error(body),
        NodeKind::For {
            from,
            to,
            step,
            body,
            ..
        } => {
            ast_has_error(from)
                || ast_has_error(to)
                || step.as_ref().is_some_and(|s| ast_has_error(s))
                || ast_has_error(body)
        }
        NodeKind::Seq(items) => items.iter().any(ast_has_error),
        NodeKind::Array(items) => items.iter().any(ast_has_error),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn and_crystal_binds_tighter_than_or() {
        let s = and_crystal("{t.y} > 5", "{t.x} = 1").expect("parse ok");
        assert_eq!(s, "{t.y} > 5 And {t.x} = 1");
    }

    #[test]
    fn stored_or_true_cannot_override_cli_and() {
        let s = and_crystal("{t.y} > 5) Or (True", "{t.x} = 1");
        assert!(s.is_err(), "unbalanced stored must not combine");
    }

    #[test]
    fn parsed_or_still_anded_semantically() {
        let s = and_crystal("False Or True", "{t.x} = 1").expect("parse ok");
        let (node, diags) = parse(&s, Syntax::Crystal);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(matches!(
            node.kind,
            NodeKind::Binary {
                op: op::AND,
                ..
            }
        ));
    }

    #[test]
    fn parse_checked_rejects_unclosed_paren() {
        assert!(parse_checked("{t.x} = (", "x").is_err());
    }
}
