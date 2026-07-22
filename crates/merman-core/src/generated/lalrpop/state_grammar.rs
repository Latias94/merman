// auto-generated: "lalrpop 0.23.1"
// sha3: ad12e290972e53e810834ca98ff6e821e0e24f897673659316ba96ad13468b12
use crate::diagrams::state::{ClickStmt, Note, RelationStmt, StateStmt, Stmt, Tok};
use crate::SourceSpan;
#[allow(unused_extern_crates)]
extern crate lalrpop_util as ___lalrpop_util;
#[allow(unused_imports)]
use self::___lalrpop_util::state_machine as ___state_machine;
#[allow(unused_extern_crates)]
extern crate alloc;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::match_single_binding, clippy::clone_on_copy, clippy::unit_arg)]
mod ___parse___Root {

    use crate::diagrams::state::{ClickStmt, Note, RelationStmt, StateStmt, Stmt, Tok};
    use crate::SourceSpan;
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as ___lalrpop_util;
    #[allow(unused_imports)]
    use self::___lalrpop_util::state_machine as ___state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::___ToTriple;
    #[allow(dead_code)]
    pub(crate) enum ___Symbol<>
     {
        Variant0(Tok),
        Variant1(String),
        Variant2((String, String)),
        Variant3(usize),
        Variant4(Option<Stmt>),
        Variant5(alloc::vec::Vec<Option<Stmt>>),
        Variant6(Stmt),
        Variant7(Vec<Stmt>),
        Variant8(StateStmt),
        Variant9(Option<Vec<Stmt>>),
        Variant10(Option<String>),
        Variant11(()),
    }
    const ___ACTION: &[i8] = &[
        // State 0
        2, -32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 1
        2, -32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 2
        39, 0, 37, 43, 34, 0, 0, 0, 0, 0, 8, 0, 0, 0, 41, 7, 35, 38, 29, 32, 36, 40, 31, 0, 0, 30, 0, 0, 42, 0, 0, 33, 28, 26, 27, 6, 0, 0,
        // State 3
        39, 0, 37, 43, 34, 0, 0, 0, -23, 0, 8, 0, 0, 0, 41, 7, 35, 38, 29, 32, 36, 40, 31, 0, 0, 30, 0, 0, 42, 0, 0, 33, 28, 26, 27, 6, 0, 0,
        // State 4
        -30, 0, -30, -30, -30, 46, 9, 0, -30, 0, -30, 0, 0, 0, -30, -30, -30, -30, -30, -30, -30, -30, -30, 0, 0, -30, 0, 0, -30, 0, 0, -30, -30, -30, -30, -30, 0, 0,
        // State 5
        0, 0, 37, 43, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 6
        -46, 0, -46, -46, -46, 0, 0, 10, -46, 0, -46, 0, 0, 0, -46, -46, -46, -46, -46, -46, -46, -46, -46, 0, 0, -46, 0, 0, -46, 0, 0, -46, -46, -46, -46, -46, 0, 0,
        // State 7
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 52, 54, 53, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 8
        0, 0, 37, 43, 34, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 9
        39, 0, 37, 43, 34, 0, 0, 0, -22, 0, 8, 0, 0, 0, 41, 7, 35, 38, 29, 32, 36, 40, 31, 0, 0, 30, 0, 0, 42, 0, 0, 33, 28, 26, 27, 6, 0, 0,
        // State 10
        -30, 0, -30, -30, -30, 46, 0, 0, -30, 0, -30, 0, 0, 0, -30, -30, -30, -30, -30, -30, -30, -30, -30, 0, 0, -30, 0, 0, -30, 0, 0, -30, -30, -30, -30, -30, 0, 0,
        // State 11
        -28, 0, -28, -28, -28, 0, 0, 10, -28, 0, -28, 0, 0, 0, -28, -28, -28, -28, -28, -28, -28, -28, -28, 0, 0, -28, 0, 0, -28, 0, 0, -28, -28, -28, -28, -28, 0, 0,
        // State 12
        0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 13
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 14
        0, -33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 15
        -39, 0, -39, -39, -39, 0, 0, 0, -39, 0, -39, 0, 0, 0, -39, -39, -39, -39, -39, -39, -39, -39, -39, 0, 0, -39, 0, 0, -39, 0, 0, -39, -39, -39, -39, -39, 0, 0,
        // State 16
        -35, 0, -35, -35, -35, 0, 0, 0, -35, 0, -35, 0, 0, 0, -35, -35, -35, -35, -35, -35, -35, -35, -35, 0, 0, -35, 0, 0, -35, 0, 0, -35, -35, -35, -35, -35, 0, 0,
        // State 17
        -40, 0, -40, -40, -40, 0, 0, 0, -40, 0, -40, 0, 0, 0, -40, -40, -40, -40, -40, -40, -40, -40, -40, 0, 0, -40, 0, 0, -40, 0, 0, -40, -40, -40, -40, -40, 0, 0,
        // State 18
        -37, 0, -37, -37, -37, 0, 0, 0, -37, 0, -37, 0, 0, 0, -37, -37, -37, -37, -37, -37, -37, -37, -37, 0, 0, -37, 0, 0, -37, 0, 0, -37, -37, -37, -37, -37, 0, 0,
        // State 19
        -38, 0, -38, -38, -38, 0, 0, 0, -38, 0, -38, 0, 0, 0, -38, -38, -38, -38, -38, -38, -38, -38, -38, 0, 0, -38, 0, 0, -38, 0, 0, -38, -38, -38, -38, -38, 0, 0,
        // State 20
        -4, 0, -4, -4, -4, 0, 0, 0, -4, 0, -4, 0, 0, 0, -4, -4, -4, -4, -4, -4, -4, -4, -4, 0, 0, -4, 0, 0, -4, 0, 0, -4, -4, -4, -4, -4, 0, 0,
        // State 21
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 22
        -41, 0, -41, -41, -41, 0, 0, 0, -41, 0, -41, 0, 0, 0, -41, -41, -41, -41, -41, -41, -41, -41, -41, 0, 0, -41, 0, 0, -41, 0, 0, -41, -41, -41, -41, -41, 0, 0,
        // State 23
        -21, 0, -21, -21, -21, 0, 0, 0, -21, 0, -21, 0, 0, 0, -21, -21, -21, -21, -21, -21, -21, -21, -21, 0, 0, -21, 0, 0, -21, 0, 0, -21, -21, -21, -21, -21, 0, 0,
        // State 24
        -36, 0, -36, -36, -36, 0, 0, 0, -36, 0, -36, 0, 0, 0, -36, -36, -36, -36, -36, -36, -36, -36, -36, 0, 0, -36, 0, 0, -36, 0, 0, -36, -36, -36, -36, -36, 0, 0,
        // State 25
        -9, 0, -9, -9, -9, 0, 0, 0, -9, 0, -9, 0, 0, 0, -9, -9, -9, -9, -9, -9, -9, -9, -9, 0, 0, -9, 0, 0, -9, 0, 0, -9, -9, -9, -9, -9, 0, 0,
        // State 26
        -10, 0, -10, -10, -10, 0, 0, 0, -10, 0, -10, 0, 0, 0, -10, -10, -10, -10, -10, -10, -10, -10, -10, 0, 0, -10, 0, 0, -10, 0, 0, -10, -10, -10, -10, -10, 0, 0,
        // State 27
        -8, 0, -8, -8, -8, 0, 0, 0, -8, 0, -8, 0, 0, 0, -8, -8, -8, -8, -8, -8, -8, -8, -8, 0, 0, -8, 0, 0, -8, 0, 0, -8, -8, -8, -8, -8, 0, 0,
        // State 28
        -51, 0, -51, -51, -51, 0, 0, 0, -51, 0, -51, 0, 0, 0, -51, -51, -51, -51, -51, -51, -51, -51, -51, 0, 0, -51, 0, 0, -51, 0, 0, -51, -51, -51, -51, -51, 0, 0,
        // State 29
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 47, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 30
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 48, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 31
        -52, 0, -52, -52, -52, 0, 0, 0, -52, 0, -52, 0, 0, 0, -52, -52, -52, -52, -52, -52, -52, -52, -52, 0, 0, -52, 0, 0, -52, 0, 0, -52, -52, -52, -52, -52, 0, 0,
        // State 32
        -16, 0, -16, -16, -16, 0, 0, 0, -16, 0, -16, 0, 0, 0, -16, -16, -16, -16, -16, -16, -16, -16, -16, 0, 0, -16, 0, 0, -16, 0, 0, -16, -16, -16, -16, -16, 0, 0,
        // State 33
        -18, 0, -18, -18, -18, -18, -18, 0, -18, 0, -18, 0, 0, 0, -18, -18, -18, -18, -18, -18, -18, -18, -18, 0, 0, -18, 0, 0, -18, 0, 0, -18, -18, -18, -18, -18, -18, -18,
        // State 34
        -49, 0, -49, -49, -49, 0, 0, 0, -49, 0, -49, 0, 0, 0, -49, -49, -49, -49, -49, -49, -49, -49, -49, 0, 0, -49, 0, 0, -49, 0, 0, -49, -49, -49, -49, -49, 0, 0,
        // State 35
        -42, 0, -42, -42, -42, 0, 0, 0, -42, 0, -42, 0, 0, 0, -42, -42, -42, -42, -42, -42, -42, -42, -42, 0, 0, -42, 0, 0, -42, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 36
        -17, 0, -17, -17, -17, -17, -17, 0, -17, 0, -17, 0, 0, 0, -17, -17, -17, -17, -17, -17, -17, -17, -17, 0, 0, -17, 0, 0, -17, 0, 0, -17, -17, -17, -17, -17, -17, -17,
        // State 37
        -50, 0, -50, -50, -50, 0, 0, 0, -50, 0, -50, 0, 0, 0, -50, -50, -50, -50, -50, -50, -50, -50, -50, 0, 0, -50, 0, 0, -50, 0, 0, -50, -50, -50, -50, -50, 0, 0,
        // State 38
        -20, 0, -20, -20, -20, 0, 0, 0, -20, 0, -20, 0, 0, 0, -20, -20, -20, -20, -20, -20, -20, -20, -20, 0, 0, -20, 0, 0, -20, 0, 0, -20, -20, -20, -20, -20, 0, 0,
        // State 39
        -43, 0, -43, -43, -43, 0, 0, 0, -43, 0, -43, 0, 0, 0, -43, -43, -43, -43, -43, -43, -43, -43, -43, 0, 0, -43, 0, 0, -43, 0, 0, -43, -43, -43, -43, -43, 0, 0,
        // State 40
        0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 41
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 56, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 42
        -19, 0, -19, -19, -19, -19, -19, 0, -19, 0, -19, 0, 0, 0, -19, -19, -19, -19, -19, -19, -19, -19, -19, 0, 0, -19, 0, 0, -19, 0, 0, -19, -19, -19, -19, -19, -19, -19,
        // State 43
        -5, 0, -5, -5, -5, 0, 0, 0, -5, 0, -5, 0, 0, 0, -5, -5, -5, -5, -5, -5, -5, -5, -5, 0, 0, -5, 0, 0, -5, 0, 0, -5, -5, -5, -5, -5, 0, 0,
        // State 44
        -45, 0, -45, -45, -45, 0, 0, 0, -45, 0, -45, 0, 0, 0, -45, -45, -45, -45, -45, -45, -45, -45, -45, 0, 0, -45, 0, 0, -45, 0, 0, -45, -45, -45, -45, -45, 0, 0,
        // State 45
        -31, 0, -31, -31, -31, 0, 0, 0, -31, 0, -31, 0, 0, 0, -31, -31, -31, -31, -31, -31, -31, -31, -31, 0, 0, -31, 0, 0, -31, 0, 0, -31, -31, -31, -31, -31, 0, 0,
        // State 46
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 57, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 47
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 48
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 59, 60,
        // State 49
        -47, 0, -47, -47, -47, 0, 0, 0, -47, 0, -47, 0, 0, 0, -47, -47, -47, -47, -47, -47, -47, -47, -47, 0, 0, -47, 0, 0, -47, 0, 0, -47, -47, -47, -47, -47, 0, 0,
        // State 50
        0, 0, 62, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 51
        0, 0, -24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 52
        0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 53
        0, 0, -25, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 54
        0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 55
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0,
        // State 56
        -15, 0, -15, -15, -15, 0, 0, 0, -15, 0, -15, 0, 0, 0, -15, -15, -15, -15, -15, -15, -15, -15, -15, 0, 0, -15, 0, 0, -15, 0, 0, -15, -15, -15, -15, -15, 0, 0,
        // State 57
        -12, 0, -12, -12, -12, 0, 0, 0, -12, 0, -12, 0, 0, 0, -12, -12, -12, -12, -12, -12, -12, -12, -12, 0, 0, -12, 0, 0, -12, 0, 0, -12, -12, -12, -12, -12, 0, 0,
        // State 58
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 66,
        // State 59
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 67,
        // State 60
        0, 0, 0, 0, 0, 0, 0, 0, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 61
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 62
        0, 0, 70, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 63
        -53, 0, -53, -53, -53, 0, 0, 0, -53, 0, -53, 0, 0, 0, -53, -53, -53, -53, -53, -53, -53, -53, -53, 0, 0, -53, 0, 0, -53, 0, 0, -53, -53, -53, -53, -53, 0, 0,
        // State 64
        -44, 0, -44, -44, -44, 0, 0, 0, -44, 0, -44, 0, 0, 0, -44, -44, -44, -44, -44, -44, -44, -44, -44, 0, 0, -44, 0, 0, -44, 0, 0, -44, -44, -44, -44, -44, 0, 0,
        // State 65
        -14, 0, -14, -14, -14, 0, 0, 0, -14, 0, -14, 0, 0, 0, -14, -14, -14, -14, -14, -14, -14, -14, -14, 0, 0, -14, 0, 0, -14, 0, 0, -14, -14, -14, -14, -14, 0, 0,
        // State 66
        -13, 0, -13, -13, -13, 0, 0, 0, -13, 0, -13, 0, 0, 0, -13, -13, -13, -13, -13, -13, -13, -13, -13, 0, 0, -13, 0, 0, -13, 0, 0, -13, -13, -13, -13, -13, 0, 0,
        // State 67
        -11, 0, -11, -11, -11, 0, 0, 0, -11, 0, -11, 0, 0, 0, -11, -11, -11, -11, -11, -11, -11, -11, -11, 0, 0, -11, 0, 0, -11, 0, 0, -11, -11, -11, -11, -11, 0, 0,
        // State 68
        -26, 0, -26, -26, -26, 0, 0, 0, -26, 0, -26, 0, 0, 0, -26, -26, -26, -26, -26, -26, -26, -26, -26, 0, 0, -26, 0, 0, -26, 0, 0, -26, -26, -26, -26, -26, 0, 0,
        // State 69
        -27, 0, -27, -27, -27, 0, 0, 0, -27, 0, -27, 0, 0, 0, -27, -27, -27, -27, -27, -27, -27, -27, -27, 0, 0, -27, 0, 0, -27, 0, 0, -27, -27, -27, -27, -27, 0, 0,
        // State 70
        -29, 0, -29, -29, -29, 0, 0, 0, -29, 0, -29, 0, 0, 0, -29, -29, -29, -29, -29, -29, -29, -29, -29, 0, 0, -29, 0, 0, -29, 0, 0, -29, -29, -29, -29, -29, 0, 0,
        // State 71
        -48, 0, -48, -48, -48, 0, 0, 0, -48, 0, -48, 0, 0, 0, -48, -48, -48, -48, -48, -48, -48, -48, -48, 0, 0, -48, 0, 0, -48, 0, 0, -48, -48, -48, -48, -48, 0, 0,
    ];
    fn ___action(state: i8, integer: usize) -> i8 {
        ___ACTION[(state as usize) * 38 + integer]
    }
    const ___EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        0,
        // State 2
        -22,
        // State 3
        -23,
        // State 4
        -30,
        // State 5
        0,
        // State 6
        -46,
        // State 7
        0,
        // State 8
        0,
        // State 9
        0,
        // State 10
        -30,
        // State 11
        -28,
        // State 12
        0,
        // State 13
        -54,
        // State 14
        0,
        // State 15
        -39,
        // State 16
        -35,
        // State 17
        -40,
        // State 18
        -37,
        // State 19
        -38,
        // State 20
        -4,
        // State 21
        -34,
        // State 22
        -41,
        // State 23
        -21,
        // State 24
        -36,
        // State 25
        -9,
        // State 26
        -10,
        // State 27
        -8,
        // State 28
        -51,
        // State 29
        0,
        // State 30
        0,
        // State 31
        -52,
        // State 32
        -16,
        // State 33
        -18,
        // State 34
        -49,
        // State 35
        -42,
        // State 36
        -17,
        // State 37
        -50,
        // State 38
        -20,
        // State 39
        -43,
        // State 40
        0,
        // State 41
        0,
        // State 42
        -19,
        // State 43
        -5,
        // State 44
        -45,
        // State 45
        -31,
        // State 46
        0,
        // State 47
        0,
        // State 48
        0,
        // State 49
        -47,
        // State 50
        0,
        // State 51
        0,
        // State 52
        0,
        // State 53
        0,
        // State 54
        0,
        // State 55
        0,
        // State 56
        -15,
        // State 57
        -12,
        // State 58
        0,
        // State 59
        0,
        // State 60
        0,
        // State 61
        0,
        // State 62
        0,
        // State 63
        -53,
        // State 64
        -44,
        // State 65
        -14,
        // State 66
        -13,
        // State 67
        -11,
        // State 68
        -26,
        // State 69
        -27,
        // State 70
        -29,
        // State 71
        -48,
    ];
    fn ___goto(state: i8, nt: usize) -> i8 {
        match nt {
            2 => 3,
            5 => 15,
            6 => match state {
                11 => 70,
                _ => 49,
            },
            7 => 16,
            8 => 17,
            9 => 18,
            10 => 19,
            11 => match state {
                8 => 10,
                5 => 48,
                _ => 4,
            },
            12 => match state {
                3 => 43,
                _ => 20,
            },
            13 => match state {
                9 => 60,
                _ => 21,
            },
            14 => 50,
            15 => 22,
            16 => 71,
            17 => match state {
                10 => 64,
                _ => 44,
            },
            18 => match state {
                1 => 14,
                _ => 12,
            },
            19 => 13,
            20 => 23,
            21 => 24,
            _ => 0,
        }
    }
    #[allow(clippy::needless_raw_string_hashes)]
    const ___TERMINAL: &[&str] = &[
        r###"Newline"###,
        r###""stateDiagram""###,
        r###"Id"###,
        r###"StyledId"###,
        r###"EdgeState"###,
        r###"Descr"###,
        r###""-->""###,
        r###""{""###,
        r###""}""###,
        r###""as""###,
        r###"Note"###,
        r###"LeftOf"###,
        r###"RightOf"###,
        r###"NoteTextTok"###,
        r###"StateDescr"###,
        r###"CompositState"###,
        r###"Fork"###,
        r###"Join"###,
        r###"Choice"###,
        r###"Concurrent"###,
        r###"HideEmptyDescription"###,
        r###"ScaleWidth"###,
        r###"ClassDef"###,
        r###"ClassDefId"###,
        r###"ClassDefStyleOpts"###,
        r###"Class"###,
        r###"ClassEntityIds"###,
        r###"StyleClass"###,
        r###"Style"###,
        r###"StyleIds"###,
        r###"StyleDefStyleOpts"###,
        r###"Direction"###,
        r###"AccTitle"###,
        r###"AccDescr"###,
        r###"AccDescrMultiline"###,
        r###"Click"###,
        r###"Href"###,
        r###"StringLit"###,
    ];
    fn ___expected_tokens(___state: i8) -> alloc::vec::Vec<alloc::string::String> {
        ___TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = ___action(___state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn ___expected_tokens_from_states<
    >(
        ___states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        ___TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if ___accepts(None, ___states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct ___StateMachine<>
    where
    {
        ___phantom: core::marker::PhantomData<()>,
    }
    impl<> ___state_machine::ParserDefinition for ___StateMachine<>
    where
    {
        type Location = usize;
        type Error = crate::diagrams::state::LexError;
        type Token = Tok;
        type TokenIndex = usize;
        type Symbol = ___Symbol<>;
        type Success = Vec<Stmt>;
        type StateIndex = i8;
        type Action = i8;
        type ReduceIndex = i8;
        type NonterminalIndex = usize;

        #[inline]
        fn start_location(&self) -> Self::Location {
              Default::default()
        }

        #[inline]
        fn start_state(&self) -> Self::StateIndex {
              0
        }

        #[inline]
        fn token_to_index(&self, token: &Self::Token) -> Option<usize> {
            ___token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            ___action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            ___action(state, 38 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            ___EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            ___goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            ___token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            ___expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            ___expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: ___state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<___state_machine::SymbolTriple<Self>>,
        ) -> Option<___state_machine::ParseResult<Self>> {
            ___reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> ___state_machine::SimulatedReduce<Self> {
            ___simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn ___token_to_integer<
    >(
        ___token: &Tok,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match ___token {
            Tok::Newline if true => Some(0),
            Tok::Sd if true => Some(1),
            Tok::Id(_) if true => Some(2),
            Tok::StyledId(_) if true => Some(3),
            Tok::EdgeState if true => Some(4),
            Tok::Descr(_) if true => Some(5),
            Tok::Arrow if true => Some(6),
            Tok::StructStart if true => Some(7),
            Tok::StructStop if true => Some(8),
            Tok::As if true => Some(9),
            Tok::Note if true => Some(10),
            Tok::LeftOf if true => Some(11),
            Tok::RightOf if true => Some(12),
            Tok::NoteText(_) if true => Some(13),
            Tok::StateDescr(_) if true => Some(14),
            Tok::CompositState(_) if true => Some(15),
            Tok::Fork(_) if true => Some(16),
            Tok::Join(_) if true => Some(17),
            Tok::Choice(_) if true => Some(18),
            Tok::Concurrent if true => Some(19),
            Tok::HideEmptyDescription if true => Some(20),
            Tok::ScaleWidth(_) if true => Some(21),
            Tok::ClassDef if true => Some(22),
            Tok::ClassDefId(_) if true => Some(23),
            Tok::ClassDefStyleOpts(_) if true => Some(24),
            Tok::Class if true => Some(25),
            Tok::ClassEntityIds(_) if true => Some(26),
            Tok::StyleClass(_) if true => Some(27),
            Tok::Style if true => Some(28),
            Tok::StyleIds(_) if true => Some(29),
            Tok::StyleDefStyleOpts(_) if true => Some(30),
            Tok::Direction(_) if true => Some(31),
            Tok::AccTitle(_) if true => Some(32),
            Tok::AccDescr(_) if true => Some(33),
            Tok::AccDescrMultiline(_) if true => Some(34),
            Tok::Click if true => Some(35),
            Tok::Href if true => Some(36),
            Tok::StringLit(_) if true => Some(37),
            _ => None,
        }
    }
    fn ___token_to_symbol<
    >(
        ___token_index: usize,
        ___token: Tok,
        _: core::marker::PhantomData<()>,
    ) -> ___Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match ___token_index {
            0 | 1 | 4 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 19 | 20 | 22 | 25 | 28 | 35 | 36 => ___Symbol::Variant0(___token),
            2 | 5 | 13 | 14 | 15 | 16 | 17 | 18 | 23 | 24 | 26 | 27 | 29 | 30 | 31 | 32 | 33 | 34 | 37 => match ___token {
                Tok::Id(___tok0) | Tok::Descr(___tok0) | Tok::NoteText(___tok0) | Tok::StateDescr(___tok0) | Tok::CompositState(___tok0) | Tok::Fork(___tok0) | Tok::Join(___tok0) | Tok::Choice(___tok0) | Tok::ClassDefId(___tok0) | Tok::ClassDefStyleOpts(___tok0) | Tok::ClassEntityIds(___tok0) | Tok::StyleClass(___tok0) | Tok::StyleIds(___tok0) | Tok::StyleDefStyleOpts(___tok0) | Tok::Direction(___tok0) | Tok::AccTitle(___tok0) | Tok::AccDescr(___tok0) | Tok::AccDescrMultiline(___tok0) | Tok::StringLit(___tok0) if true => ___Symbol::Variant1(___tok0),
                _ => unreachable!(),
            },
            3 => match ___token {
                Tok::StyledId(___tok0) if true => ___Symbol::Variant2(___tok0),
                _ => unreachable!(),
            },
            21 => match ___token {
                Tok::ScaleWidth(___tok0) if true => ___Symbol::Variant3(___tok0),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
    fn ___simulate_reduce<
    >(
        ___reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> ___state_machine::SimulatedReduce<___StateMachine<>>
    {
        match ___reduce_index {
            0 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 1,
                }
            }
            2 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 1,
                }
            }
            3 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 2,
                }
            }
            4 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            5 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 3,
                }
            }
            6 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            7 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            8 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            9 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            10 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 6,
                }
            }
            11 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 7,
                }
            }
            12 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 8,
                }
            }
            13 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 8,
                }
            }
            14 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 9,
                }
            }
            15 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            16 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            17 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            18 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            19 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            20 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 12,
                }
            }
            21 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 13,
                }
            }
            22 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 13,
                }
            }
            23 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 14,
                }
            }
            24 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 14,
                }
            }
            25 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 15,
                }
            }
            26 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 15,
                }
            }
            27 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 16,
                }
            }
            28 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 16,
                }
            }
            29 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 17,
                }
            }
            30 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 17,
                }
            }
            31 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 18,
                }
            }
            32 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 18,
                }
            }
            33 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 19,
                }
            }
            34 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            35 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            36 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            37 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            38 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            39 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            40 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            41 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            42 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            43 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 20,
                }
            }
            44 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 20,
                }
            }
            45 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            46 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 20,
                }
            }
            47 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 20,
                }
            }
            48 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            49 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            50 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            51 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            52 => {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 21,
                }
            }
            53 => ___state_machine::SimulatedReduce::Accept,
            _ => panic!("invalid reduction index {___reduce_index}")
        }
    }
    pub struct RootParser {
        _priv: (),
    }

    impl Default for RootParser { fn default() -> Self { Self::new() } }
    impl RootParser {
        pub fn new() -> RootParser {
            RootParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            ___TOKEN: ___ToTriple<>,
            ___TOKENS: IntoIterator<Item=___TOKEN>,
        >(
            &self,
            ___tokens0: ___TOKENS,
        ) -> Result<Vec<Stmt>, ___lalrpop_util::ParseError<usize, Tok, crate::diagrams::state::LexError>>
        {
            let ___tokens = ___tokens0.into_iter();
            let mut ___tokens = ___tokens.map(|t| ___ToTriple::to_triple(t));
            ___state_machine::Parser::drive(
                ___StateMachine {
                    ___phantom: core::marker::PhantomData::<()>,
                },
                ___tokens,
            )
        }
    }
    fn ___accepts<
    >(
        ___error_state: Option<i8>,
        ___states: &[i8],
        ___opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut ___states = ___states.to_vec();
        ___states.extend(___error_state);
        loop {
            let mut ___states_len = ___states.len();
            let ___top = ___states[___states_len - 1];
            let ___action = match ___opt_integer {
                None => ___EOF_ACTION[___top as usize],
                Some(___integer) => ___action(___top, ___integer),
            };
            if ___action == 0 { return false; }
            if ___action > 0 { return true; }
            let (___to_pop, ___nt) = match ___simulate_reduce(-(___action + 1), core::marker::PhantomData::<()>) {
                ___state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                ___state_machine::SimulatedReduce::Accept => return true,
            };
            ___states_len -= ___to_pop;
            ___states.truncate(___states_len);
            let ___top = ___states[___states_len - 1];
            let ___next_state = ___goto(___top, ___nt);
            ___states.push(___next_state);
        }
    }
    fn ___reduce<
    >(
        ___action: i8,
        ___lookahead_start: Option<&usize>,
        ___states: &mut alloc::vec::Vec<i8>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<Vec<Stmt>,___lalrpop_util::ParseError<usize, Tok, crate::diagrams::state::LexError>>>
    {
        let (___pop_states, ___nonterminal) = match ___action {
            0 => {
                ___reduce0(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                ___reduce1(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                ___reduce2(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                ___reduce3(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                ___reduce4(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                ___reduce5(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                ___reduce6(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                ___reduce7(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                ___reduce8(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                ___reduce9(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                ___reduce10(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                ___reduce11(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                ___reduce12(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                ___reduce13(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                ___reduce14(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                ___reduce15(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                ___reduce16(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                ___reduce17(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                ___reduce18(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                ___reduce19(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                ___reduce20(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                ___reduce21(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                ___reduce22(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                ___reduce23(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                ___reduce24(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                ___reduce25(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                ___reduce26(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                ___reduce27(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            28 => {
                ___reduce28(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            29 => {
                ___reduce29(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            30 => {
                ___reduce30(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            31 => {
                ___reduce31(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            32 => {
                ___reduce32(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            33 => {
                ___reduce33(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            34 => {
                ___reduce34(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            35 => {
                ___reduce35(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            36 => {
                ___reduce36(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            37 => {
                ___reduce37(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            38 => {
                ___reduce38(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            39 => {
                ___reduce39(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            40 => {
                ___reduce40(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            41 => {
                ___reduce41(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            42 => {
                ___reduce42(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            43 => {
                ___reduce43(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            44 => {
                ___reduce44(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            45 => {
                ___reduce45(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            46 => {
                ___reduce46(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            47 => {
                ___reduce47(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            48 => {
                ___reduce48(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            49 => {
                ___reduce49(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            50 => {
                ___reduce50(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            51 => {
                ___reduce51(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            52 => {
                ___reduce52(___lookahead_start, ___symbols, core::marker::PhantomData::<()>)
            }
            53 => {
                // ___Root = Root => ActionFn(0);
                let ___sym0 = ___pop_Variant7(___symbols);
                let ___start = ___sym0.0.clone();
                let ___end = ___sym0.2.clone();
                let ___nt = super::___action0::<>(___sym0);
                return Some(Ok(___nt));
            }
            _ => panic!("invalid action code {___action}")
        };
        let ___states_len = ___states.len();
        ___states.truncate(___states_len - ___pop_states);
        let ___state = *___states.last().unwrap();
        let ___next_state = ___goto(___state, ___nonterminal);
        ___states.push(___next_state);
        None
    }
    #[inline(never)]
    fn ___symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn ___pop_Variant11<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, (), usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant11(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant2<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, (String, String), usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant2(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant4<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Option<Stmt>, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant4(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant10<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Option<String>, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant10(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant9<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Option<Vec<Stmt>>, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant9(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant8<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, StateStmt, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant8(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant6<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Stmt, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant6(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant1<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, String, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant1(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant0<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Tok, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant0(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant7<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, Vec<Stmt>, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant7(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant5<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, alloc::vec::Vec<Option<Stmt>>, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant5(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___pop_Variant3<
    >(
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>
    ) -> (usize, usize, usize)
     {
        match ___symbols.pop() {
            Some((___l, ___Symbol::Variant3(___v), ___r)) => (___l, ___v, ___r),
            _ => ___symbol_type_mismatch()
        }
    }
    fn ___reduce0<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // (Item) = Item => ActionFn(50);
        let ___sym0 = ___pop_Variant4(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action50::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant4(___nt), ___end));
        (1, 0)
    }
    fn ___reduce1<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // (Item)* =  => ActionFn(48);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action48::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant5(___nt), ___end));
        (0, 1)
    }
    fn ___reduce2<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // (Item)* = (Item)+ => ActionFn(49);
        let ___sym0 = ___pop_Variant5(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action49::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant5(___nt), ___end));
        (1, 1)
    }
    fn ___reduce3<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // (Item)+ = Item => ActionFn(53);
        let ___sym0 = ___pop_Variant4(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action53::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant5(___nt), ___end));
        (1, 2)
    }
    fn ___reduce4<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // (Item)+ = (Item)+, Item => ActionFn(54);
        assert!(___symbols.len() >= 2);
        let ___sym1 = ___pop_Variant4(___symbols);
        let ___sym0 = ___pop_Variant5(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym1.2.clone();
        let ___nt = super::___action54::<>(___sym0, ___sym1);
        ___symbols.push((___start, ___Symbol::Variant5(___nt), ___end));
        (2, 2)
    }
    fn ___reduce5<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // @L =  => ActionFn(47);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action47::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant3(___nt), ___end));
        (0, 3)
    }
    fn ___reduce6<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // @R =  => ActionFn(46);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action46::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant3(___nt), ___end));
        (0, 4)
    }
    fn ___reduce7<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AccessibilityStatement = AccTitle => ActionFn(38);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action38::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 5)
    }
    fn ___reduce8<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AccessibilityStatement = AccDescr => ActionFn(39);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action39::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 5)
    }
    fn ___reduce9<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // AccessibilityStatement = AccDescrMultiline => ActionFn(40);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action40::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 5)
    }
    fn ___reduce10<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Block = "{", Items, "}" => ActionFn(31);
        assert!(___symbols.len() >= 3);
        let ___sym2 = ___pop_Variant0(___symbols);
        let ___sym1 = ___pop_Variant7(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym2.2.clone();
        let ___nt = super::___action31::<>(___sym0, ___sym1, ___sym2);
        ___symbols.push((___start, ___Symbol::Variant7(___nt), ___end));
        (3, 6)
    }
    fn ___reduce11<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ClassDefStatement = ClassDef, ClassDefId, ClassDefStyleOpts => ActionFn(34);
        assert!(___symbols.len() >= 3);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant1(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym2.2.clone();
        let ___nt = super::___action34::<>(___sym0, ___sym1, ___sym2);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (3, 7)
    }
    fn ___reduce12<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ClickStatement = Click, IdStatement, StringLit, StringLit => ActionFn(41);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant1(___symbols);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant8(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action41::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 8)
    }
    fn ___reduce13<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ClickStatement = Click, IdStatement, Href, StringLit => ActionFn(42);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant1(___symbols);
        let ___sym2 = ___pop_Variant0(___symbols);
        let ___sym1 = ___pop_Variant8(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action42::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 8)
    }
    fn ___reduce14<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // CssClassStatement = Class, ClassEntityIds, StyleClass => ActionFn(36);
        assert!(___symbols.len() >= 3);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant1(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym2.2.clone();
        let ___nt = super::___action36::<>(___sym0, ___sym1, ___sym2);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (3, 9)
    }
    fn ___reduce15<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // DirectionStatement = Direction => ActionFn(37);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action37::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 10)
    }
    fn ___reduce16<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // IdStatement = Id => ActionFn(66);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action66::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant8(___nt), ___end));
        (1, 11)
    }
    fn ___reduce17<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // IdStatement = EdgeState => ActionFn(67);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action67::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant8(___nt), ___end));
        (1, 11)
    }
    fn ___reduce18<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // IdStatement = StyledId => ActionFn(59);
        let ___sym0 = ___pop_Variant2(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action59::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant8(___nt), ___end));
        (1, 11)
    }
    fn ___reduce19<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Item = Newline => ActionFn(5);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action5::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant4(___nt), ___end));
        (1, 12)
    }
    fn ___reduce20<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Item = Statement => ActionFn(6);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action6::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant4(___nt), ___end));
        (1, 12)
    }
    fn ___reduce21<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Items =  => ActionFn(55);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action55::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant7(___nt), ___end));
        (0, 13)
    }
    fn ___reduce22<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Items = (Item)+ => ActionFn(56);
        let ___sym0 = ___pop_Variant5(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action56::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant7(___nt), ___end));
        (1, 13)
    }
    fn ___reduce23<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NotePosition = LeftOf => ActionFn(27);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action27::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant1(___nt), ___end));
        (1, 14)
    }
    fn ___reduce24<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NotePosition = RightOf => ActionFn(28);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action28::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant1(___nt), ___end));
        (1, 14)
    }
    fn ___reduce25<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NoteStatement = Note, NotePosition, Id, NoteTextTok => ActionFn(68);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant1(___symbols);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant1(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action68::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 15)
    }
    fn ___reduce26<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NoteStatement = Note, NoteTextTok, "as", Id => ActionFn(26);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant1(___symbols);
        let ___sym2 = ___pop_Variant0(___symbols);
        let ___sym1 = ___pop_Variant1(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action26::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 15)
    }
    fn ___reduce27<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // OptBlock =  => ActionFn(32);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action32::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant9(___nt), ___end));
        (0, 16)
    }
    fn ___reduce28<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // OptBlock = Block => ActionFn(33);
        let ___sym0 = ___pop_Variant7(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action33::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant9(___nt), ___end));
        (1, 16)
    }
    fn ___reduce29<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // OptDescr =  => ActionFn(29);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action29::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant10(___nt), ___end));
        (0, 17)
    }
    fn ___reduce30<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // OptDescr = Descr => ActionFn(30);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action30::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant10(___nt), ___end));
        (1, 17)
    }
    fn ___reduce31<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Prelude =  => ActionFn(2);
        let ___start = ___lookahead_start.cloned().or_else(|| ___symbols.last().map(|s| s.2)).unwrap_or_default();
        let ___end = ___start;
        let ___nt = super::___action2::<>(&___start, &___end);
        ___symbols.push((___start, ___Symbol::Variant11(___nt), ___end));
        (0, 18)
    }
    fn ___reduce32<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Prelude = Newline, Prelude => ActionFn(3);
        assert!(___symbols.len() >= 2);
        let ___sym1 = ___pop_Variant11(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym1.2.clone();
        let ___nt = super::___action3::<>(___sym0, ___sym1);
        ___symbols.push((___start, ___Symbol::Variant11(___nt), ___end));
        (2, 18)
    }
    fn ___reduce33<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Root = Prelude, "stateDiagram", Items => ActionFn(1);
        assert!(___symbols.len() >= 3);
        let ___sym2 = ___pop_Variant7(___symbols);
        let ___sym1 = ___pop_Variant0(___symbols);
        let ___sym0 = ___pop_Variant11(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym2.2.clone();
        let ___nt = super::___action1::<>(___sym0, ___sym1, ___sym2);
        ___symbols.push((___start, ___Symbol::Variant7(___nt), ___end));
        (3, 19)
    }
    fn ___reduce34<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ClassDefStatement => ActionFn(7);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action7::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce35<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = StyleStatement => ActionFn(8);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action8::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce36<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = CssClassStatement => ActionFn(9);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action9::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce37<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = DirectionStatement => ActionFn(10);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action10::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce38<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = AccessibilityStatement => ActionFn(11);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action11::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce39<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ClickStatement => ActionFn(12);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action12::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce40<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = NoteStatement => ActionFn(13);
        let ___sym0 = ___pop_Variant6(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action13::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce41<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = HideEmptyDescription => ActionFn(14);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action14::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce42<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ScaleWidth => ActionFn(15);
        let ___sym0 = ___pop_Variant3(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action15::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce43<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = IdStatement, "-->", IdStatement, OptDescr => ActionFn(16);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant10(___symbols);
        let ___sym2 = ___pop_Variant8(___symbols);
        let ___sym1 = ___pop_Variant0(___symbols);
        let ___sym0 = ___pop_Variant8(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action16::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 20)
    }
    fn ___reduce44<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = IdStatement, OptDescr => ActionFn(17);
        assert!(___symbols.len() >= 2);
        let ___sym1 = ___pop_Variant10(___symbols);
        let ___sym0 = ___pop_Variant8(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym1.2.clone();
        let ___nt = super::___action17::<>(___sym0, ___sym1);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (2, 20)
    }
    fn ___reduce45<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = CompositState => ActionFn(18);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action18::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce46<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = CompositState, Block => ActionFn(69);
        assert!(___symbols.len() >= 2);
        let ___sym1 = ___pop_Variant7(___symbols);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym1.2.clone();
        let ___nt = super::___action69::<>(___sym0, ___sym1);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (2, 20)
    }
    fn ___reduce47<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = StateDescr, "as", Id, OptBlock => ActionFn(62);
        assert!(___symbols.len() >= 4);
        let ___sym3 = ___pop_Variant9(___symbols);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant0(___symbols);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym3.2.clone();
        let ___nt = super::___action62::<>(___sym0, ___sym1, ___sym2, ___sym3);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (4, 20)
    }
    fn ___reduce48<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Fork => ActionFn(63);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action63::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce49<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Join => ActionFn(64);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action64::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce50<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Choice => ActionFn(65);
        let ___sym0 = ___pop_Variant1(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action65::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce51<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Concurrent => ActionFn(24);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym0.2.clone();
        let ___nt = super::___action24::<>(___sym0);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (1, 20)
    }
    fn ___reduce52<
    >(
        ___lookahead_start: Option<&usize>,
        ___symbols: &mut alloc::vec::Vec<(usize,___Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StyleStatement = Style, StyleIds, StyleDefStyleOpts => ActionFn(35);
        assert!(___symbols.len() >= 3);
        let ___sym2 = ___pop_Variant1(___symbols);
        let ___sym1 = ___pop_Variant1(___symbols);
        let ___sym0 = ___pop_Variant0(___symbols);
        let ___start = ___sym0.0.clone();
        let ___end = ___sym2.2.clone();
        let ___nt = super::___action35::<>(___sym0, ___sym1, ___sym2);
        ___symbols.push((___start, ___Symbol::Variant6(___nt), ___end));
        (3, 21)
    }
}
#[allow(unused_imports)]
pub use self::___parse___Root::RootParser;

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action0<
>(
    (_, ___0, _): (usize, Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    ___0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action1<
>(
    (_, _p, _): (usize, (), usize),
    (_, _, _): (usize, Tok, usize),
    (_, items, _): (usize, Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    items
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action2<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
)
{
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action3<
>(
    (_, _, _): (usize, Tok, usize),
    (_, rest, _): (usize, (), usize),
)
{
    rest
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action4<
>(
    (_, items, _): (usize, alloc::vec::Vec<Option<Stmt>>, usize),
) -> Vec<Stmt>
{
    items.into_iter().filter_map(|i| i).collect()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action5<
>(
    (_, ___0, _): (usize, Tok, usize),
) -> Option<Stmt>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action6<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Option<Stmt>
{
    Some(s)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action7<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action8<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action9<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action10<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action11<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action12<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action13<
>(
    (_, s, _): (usize, Stmt, usize),
) -> Stmt
{
    s
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action14<
>(
    (_, ___0, _): (usize, Tok, usize),
) -> Stmt
{
    Stmt::Noop
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action15<
>(
    (_, ___0, _): (usize, usize, usize),
) -> Stmt
{
    Stmt::Noop
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action16<
>(
    (_, s1, _): (usize, StateStmt, usize),
    (_, _, _): (usize, Tok, usize),
    (_, s2, _): (usize, StateStmt, usize),
    (_, d, _): (usize, Option<String>, usize),
) -> Stmt
{
    Stmt::Relation(Box::new(RelationStmt {
    state1: s1,
    state2: s2,
    description: d,
  }))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action17<
>(
    (_, mut s, _): (usize, StateStmt, usize),
    (_, d, _): (usize, Option<String>, usize),
) -> Stmt
{
    {
    s.description = d;
    Stmt::State(s)
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action18<
>(
    (_, _id, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Noop
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action19<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, doc, _): (usize, Vec<Stmt>, usize),
) -> Stmt
{
    Stmt::State(StateStmt {
    id,
    id_span: Some(SourceSpan::new(l, r)),
    ty: "default".to_string(),
    description: None,
    descriptions: Vec::new(),
    doc: Some(doc),
    note: None,
    classes: Vec::new(),
    styles: Vec::new(),
    text_styles: Vec::new(),
    start: None,
  })
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action20<
>(
    (_, descr, _): (usize, String, usize),
    (_, _, _): (usize, Tok, usize),
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, doc, _): (usize, Option<Vec<Stmt>>, usize),
) -> Stmt
{
    {
    let trimmed = descr.trim().to_string();
    let mut state_id = id;
    let mut id_span = Some(SourceSpan::new(l, l + state_id.len()));
    let description = trimmed;
    let mut descriptions: Vec<String> = Vec::new();
    if let Some((a, b)) = state_id
      .split_once(':')
      .map(|(a, b)| (a.to_string(), b.to_string())) {
      id_span = Some(SourceSpan::new(l, l + a.len()));
      state_id = a;
      let extra = b.trim();
      if !extra.is_empty() {
        descriptions.push(extra.to_string());
      }
    }
    Stmt::State(StateStmt {
      id: state_id,
      id_span,
      ty: "default".to_string(),
      description: Some(description),
      descriptions,
      doc,
      note: None,
      classes: Vec::new(),
      styles: Vec::new(),
      text_styles: Vec::new(),
      start: None,
    })
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action21<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
) -> Stmt
{
    {
    let mut state = StateStmt::new_typed(id, "fork");
    state.id_span = Some(SourceSpan::new(l, l + state.id.len()));
    Stmt::State(state)
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action22<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
) -> Stmt
{
    {
    let mut state = StateStmt::new_typed(id, "join");
    state.id_span = Some(SourceSpan::new(l, l + state.id.len()));
    Stmt::State(state)
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action23<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
) -> Stmt
{
    {
    let mut state = StateStmt::new_typed(id, "choice");
    state.id_span = Some(SourceSpan::new(l, l + state.id.len()));
    Stmt::State(state)
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action24<
>(
    (_, ___0, _): (usize, Tok, usize),
) -> Stmt
{
    Stmt::State(StateStmt::new_typed("__divider__".to_string(), "divider"))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action25<
>(
    (_, _, _): (usize, Tok, usize),
    (_, pos, _): (usize, String, usize),
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, text, _): (usize, String, usize),
) -> Stmt
{
    Stmt::State(StateStmt {
    id,
    id_span: Some(SourceSpan::new(l, r)),
    ty: "default".to_string(),
    description: None,
    descriptions: Vec::new(),
    doc: None,
    note: Some(Note { position: Some(pos), text }),
    classes: Vec::new(),
    styles: Vec::new(),
    text_styles: Vec::new(),
    start: None,
  })
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action26<
>(
    (_, _, _): (usize, Tok, usize),
    (_, _text, _): (usize, String, usize),
    (_, _, _): (usize, Tok, usize),
    (_, _id, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Noop
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action27<
>(
    (_, ___0, _): (usize, Tok, usize),
) -> String
{
    "left of".to_string()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action28<
>(
    (_, ___0, _): (usize, Tok, usize),
) -> String
{
    "right of".to_string()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action29<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> Option<String>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action30<
>(
    (_, d, _): (usize, String, usize),
) -> Option<String>
{
    Some(d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action31<
>(
    (_, _, _): (usize, Tok, usize),
    (_, doc, _): (usize, Vec<Stmt>, usize),
    (_, _, _): (usize, Tok, usize),
) -> Vec<Stmt>
{
    doc
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action32<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> Option<Vec<Stmt>>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action33<
>(
    (_, b, _): (usize, Vec<Stmt>, usize),
) -> Option<Vec<Stmt>>
{
    Some(b)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action34<
>(
    (_, _, _): (usize, Tok, usize),
    (_, id, _): (usize, String, usize),
    (_, raw, _): (usize, String, usize),
) -> Stmt
{
    Stmt::ClassDef { id, classes: raw }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action35<
>(
    (_, _, _): (usize, Tok, usize),
    (_, ids, _): (usize, String, usize),
    (_, raw, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Style { ids, styles: raw }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action36<
>(
    (_, _, _): (usize, Tok, usize),
    (_, ids, _): (usize, String, usize),
    (_, style, _): (usize, String, usize),
) -> Stmt
{
    Stmt::ApplyClass { ids, class_name: style }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action37<
>(
    (_, d, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Direction(d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action38<
>(
    (_, t, _): (usize, String, usize),
) -> Stmt
{
    Stmt::AccTitle(t)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action39<
>(
    (_, d, _): (usize, String, usize),
) -> Stmt
{
    Stmt::AccDescr(d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action40<
>(
    (_, d, _): (usize, String, usize),
) -> Stmt
{
    Stmt::AccDescr(d)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action41<
>(
    (_, _, _): (usize, Tok, usize),
    (_, s, _): (usize, StateStmt, usize),
    (_, url, _): (usize, String, usize),
    (_, tooltip, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Click(ClickStmt {
    id: s.id,
    url,
    tooltip,
  })
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action42<
>(
    (_, _, _): (usize, Tok, usize),
    (_, s, _): (usize, StateStmt, usize),
    (_, _, _): (usize, Tok, usize),
    (_, url, _): (usize, String, usize),
) -> Stmt
{
    Stmt::Click(ClickStmt {
    id: s.id,
    url,
    tooltip: String::new(),
  })
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action43<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
) -> StateStmt
{
    {
    let mut state = StateStmt::new(id);
    state.id_span = Some(SourceSpan::new(l, r));
    state
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action44<
>(
    (_, l, _): (usize, usize, usize),
    (_, _, _): (usize, Tok, usize),
    (_, r, _): (usize, usize, usize),
) -> StateStmt
{
    {
    let mut state = StateStmt::new("[*]".to_string());
    state.id_span = Some(SourceSpan::new(l, r));
    state
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action45<
>(
    (_, l, _): (usize, usize, usize),
    (_, pair, _): (usize, (String, String), usize),
) -> StateStmt
{
    {
    let (id, class_id) = pair;
    let mut s = StateStmt::new(id);
    s.id_span = Some(SourceSpan::new(l, l + s.id.len()));
    s.classes.push(class_id);
    s
  }
}

#[allow(clippy::needless_lifetimes)]
fn ___action46<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> usize
{
    *___lookbehind
}

#[allow(clippy::needless_lifetimes)]
fn ___action47<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> usize
{
    *___lookahead
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action48<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> alloc::vec::Vec<Option<Stmt>>
{
    alloc::vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action49<
>(
    (_, v, _): (usize, alloc::vec::Vec<Option<Stmt>>, usize),
) -> alloc::vec::Vec<Option<Stmt>>
{
    v
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action50<
>(
    (_, ___0, _): (usize, Option<Stmt>, usize),
) -> Option<Stmt>
{
    ___0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action51<
>(
    (_, ___0, _): (usize, Option<Stmt>, usize),
) -> alloc::vec::Vec<Option<Stmt>>
{
    alloc::vec![___0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn ___action52<
>(
    (_, v, _): (usize, alloc::vec::Vec<Option<Stmt>>, usize),
    (_, e, _): (usize, Option<Stmt>, usize),
) -> alloc::vec::Vec<Option<Stmt>>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action53<
>(
    ___0: (usize, Option<Stmt>, usize),
) -> alloc::vec::Vec<Option<Stmt>>
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.2;
    let ___temp0 = ___action50(
        ___0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action51(
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action54<
>(
    ___0: (usize, alloc::vec::Vec<Option<Stmt>>, usize),
    ___1: (usize, Option<Stmt>, usize),
) -> alloc::vec::Vec<Option<Stmt>>
{
    let ___start0 = ___1.0;
    let ___end0 = ___1.2;
    let ___temp0 = ___action50(
        ___1,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action52(
        ___0,
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action55<
>(
    ___lookbehind: &usize,
    ___lookahead: &usize,
) -> Vec<Stmt>
{
    let ___start0 = *___lookbehind;
    let ___end0 = *___lookahead;
    let ___temp0 = ___action48(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action4(
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action56<
>(
    ___0: (usize, alloc::vec::Vec<Option<Stmt>>, usize),
) -> Vec<Stmt>
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.2;
    let ___temp0 = ___action49(
        ___0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action4(
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action57<
>(
    ___0: (usize, String, usize),
    ___1: (usize, usize, usize),
) -> StateStmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action43(
        ___temp0,
        ___0,
        ___1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action58<
>(
    ___0: (usize, Tok, usize),
    ___1: (usize, usize, usize),
) -> StateStmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action44(
        ___temp0,
        ___0,
        ___1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action59<
>(
    ___0: (usize, (String, String), usize),
) -> StateStmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action45(
        ___temp0,
        ___0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action60<
>(
    ___0: (usize, Tok, usize),
    ___1: (usize, String, usize),
    ___2: (usize, String, usize),
    ___3: (usize, usize, usize),
    ___4: (usize, String, usize),
) -> Stmt
{
    let ___start0 = ___1.2;
    let ___end0 = ___2.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action25(
        ___0,
        ___1,
        ___temp0,
        ___2,
        ___3,
        ___4,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action61<
>(
    ___0: (usize, String, usize),
    ___1: (usize, usize, usize),
    ___2: (usize, Vec<Stmt>, usize),
) -> Stmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action19(
        ___temp0,
        ___0,
        ___1,
        ___2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action62<
>(
    ___0: (usize, String, usize),
    ___1: (usize, Tok, usize),
    ___2: (usize, String, usize),
    ___3: (usize, Option<Vec<Stmt>>, usize),
) -> Stmt
{
    let ___start0 = ___1.2;
    let ___end0 = ___2.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action20(
        ___0,
        ___1,
        ___temp0,
        ___2,
        ___3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action63<
>(
    ___0: (usize, String, usize),
) -> Stmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action21(
        ___temp0,
        ___0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action64<
>(
    ___0: (usize, String, usize),
) -> Stmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action22(
        ___temp0,
        ___0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action65<
>(
    ___0: (usize, String, usize),
) -> Stmt
{
    let ___start0 = ___0.0;
    let ___end0 = ___0.0;
    let ___temp0 = ___action47(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action23(
        ___temp0,
        ___0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action66<
>(
    ___0: (usize, String, usize),
) -> StateStmt
{
    let ___start0 = ___0.2;
    let ___end0 = ___0.2;
    let ___temp0 = ___action46(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action57(
        ___0,
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action67<
>(
    ___0: (usize, Tok, usize),
) -> StateStmt
{
    let ___start0 = ___0.2;
    let ___end0 = ___0.2;
    let ___temp0 = ___action46(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action58(
        ___0,
        ___temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action68<
>(
    ___0: (usize, Tok, usize),
    ___1: (usize, String, usize),
    ___2: (usize, String, usize),
    ___3: (usize, String, usize),
) -> Stmt
{
    let ___start0 = ___2.2;
    let ___end0 = ___3.0;
    let ___temp0 = ___action46(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action60(
        ___0,
        ___1,
        ___2,
        ___temp0,
        ___3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn ___action69<
>(
    ___0: (usize, String, usize),
    ___1: (usize, Vec<Stmt>, usize),
) -> Stmt
{
    let ___start0 = ___0.2;
    let ___end0 = ___1.0;
    let ___temp0 = ___action46(
        &___start0,
        &___end0,
    );
    let ___temp0 = (___start0, ___temp0, ___end0);
    ___action61(
        ___0,
        ___temp0,
        ___1,
    )
}

#[allow(clippy::type_complexity, dead_code)]
pub trait ___ToTriple<>
{
    fn to_triple(self) -> Result<(usize,Tok,usize), ___lalrpop_util::ParseError<usize, Tok, crate::diagrams::state::LexError>>;
}

impl<> ___ToTriple<> for (usize, Tok, usize)
{
    fn to_triple(self) -> Result<(usize,Tok,usize), ___lalrpop_util::ParseError<usize, Tok, crate::diagrams::state::LexError>> {
        Ok(self)
    }
}
impl<> ___ToTriple<> for Result<(usize, Tok, usize), crate::diagrams::state::LexError>
{
    fn to_triple(self) -> Result<(usize,Tok,usize), ___lalrpop_util::ParseError<usize, Tok, crate::diagrams::state::LexError>> {
        self.map_err(|error| ___lalrpop_util::ParseError::User { error })
    }
}
