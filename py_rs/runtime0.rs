//! rewrite of py/runtime0.h (unary/binary op enums shared with bytecode)
// symmetry: done

/// Matches `mp_unary_op_t` order in py/runtime0.h.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Positive = 0,
    Negative,
    Invert,
    Not,
    Bool,
    Len,
    Hash,
    Abs,
    IntMaybe,
    FloatMaybe,
    ComplexMaybe,
    Sizeof,
}

pub const UNARY_OP_NUM_BYTECODE: u8 = (UnaryOp::Not as u8) + 1;
pub const UNARY_OP_NUM_RUNTIME: u8 = (UnaryOp::Sizeof as u8) + 1;

/// Matches `mp_binary_op_t` order in py/runtime0.h (bytecode-stable prefix first).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    // relational (bytecode) — must match C `MP_BINARY_OP_*` discriminants
    Less = 0,
    More,
    Equal,
    LessEqual,
    MoreEqual,
    NotEqual,
    In,
    Is,
    ExceptionMatch,
    // inplace (bytecode)
    InplaceOr,
    InplaceXor,
    InplaceAnd,
    InplaceLshift,
    InplaceRshift,
    InplaceAdd,
    InplaceSubtract,
    InplaceMultiply,
    InplaceMatMult,
    InplaceFloorDivide,
    InplaceTrueDivide,
    InplaceModulo,
    InplacePower,
    // arithmetic (bytecode)
    Or,
    Xor,
    And,
    Lshift,
    Rshift,
    Add,
    Subtract,
    Multiply,
    MatMult,
    FloorDivide,
    TrueDivide,
    Modulo,
    Power,
    // runtime-only
    Divmod,
    Contains,
    ReverseOr,
    ReverseXor,
    ReverseAnd,
    ReverseLshift,
    ReverseRshift,
    ReverseAdd,
    ReverseSubtract,
    ReverseMultiply,
    ReverseMatMult,
    ReverseFloorDivide,
    ReverseTrueDivide,
    ReverseModulo,
    ReversePower,
    NotIn,
    IsNot,
}

pub const BINARY_OP_NUM_BYTECODE: u8 = (BinaryOp::Power as u8) + 1;
pub const BINARY_OP_NUM_RUNTIME: u8 = (BinaryOp::ReversePower as u8) + 1;

/// VM return kind (`mp_vm_return_kind_t`).
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VmReturnKind {
    Normal = 0,
    Yield = 1,
    Exception = 2,
}

pub const CODE_STATE_EXC_SP_IDX_SENTINEL: u16 = u16::MAX;
