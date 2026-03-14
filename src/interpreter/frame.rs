use crate::parser::ast::{Expr, Stmt};

#[derive(Clone)]
pub enum Frame {
    Block {
        statements: Vec<Stmt>,
        pc: usize,
    },
    Loop {
        condition: Expr,
        body: Vec<Stmt>,
        pc: usize,
    },
    FunctionCall {
        body: Vec<Stmt>,
        pc: usize,
    },
}
