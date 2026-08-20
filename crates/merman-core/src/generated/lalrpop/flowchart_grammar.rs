// auto-generated: "lalrpop 0.23.1"
// sha3: 67f0ec0b5bc37030a161f93ebd6117c8bf77c74b1413402f983270802fa7fdb3
use crate::diagrams::flowchart::{
  ArrowToken, ClassAssignStmt, ClassDefStmt, ClickStmt, DirectionStatementToken, Edge, FlowchartAst,
  FlowNodeProvenance, FlowNodeSyntax, LabeledText, LinkStyleStmt, LinkToken, Node, NodeLabelToken,
  Stmt, StyleStmt, SubgraphBlock, SubgraphHeader, TitleKind, Tok
};
use crate::SourceSpan;
#[allow(unused_extern_crates)]
extern crate lalrpop_util as __lalrpop_util;
#[allow(unused_imports)]
use self::__lalrpop_util::state_machine as __state_machine;
#[allow(unused_extern_crates)]
extern crate alloc;

#[rustfmt::skip]
#[allow(explicit_outlives_requirements, non_snake_case, non_camel_case_types, unused_mut, unused_variables, unused_imports, unused_parens, clippy::needless_lifetimes, clippy::type_complexity, clippy::needless_return, clippy::too_many_arguments, clippy::match_single_binding, clippy::clone_on_copy, clippy::unit_arg)]
mod __parse__FlowchartAst {

    use crate::diagrams::flowchart::{
  ArrowToken, ClassAssignStmt, ClassDefStmt, ClickStmt, DirectionStatementToken, Edge, FlowchartAst,
  FlowNodeProvenance, FlowNodeSyntax, LabeledText, LinkStyleStmt, LinkToken, Node, NodeLabelToken,
  Stmt, StyleStmt, SubgraphBlock, SubgraphHeader, TitleKind, Tok
};
    use crate::SourceSpan;
    #[allow(unused_extern_crates)]
    extern crate lalrpop_util as __lalrpop_util;
    #[allow(unused_imports)]
    use self::__lalrpop_util::state_machine as __state_machine;
    #[allow(unused_extern_crates)]
    extern crate alloc;
    use super::__ToTriple;
    #[allow(dead_code)]
    pub(crate) enum __Symbol<>
     {
        Variant0(Tok),
        Variant1(String),
        Variant2(DirectionStatementToken),
        Variant3(NodeLabelToken),
        Variant4(ArrowToken),
        Variant5(LabeledText),
        Variant6(SubgraphHeader),
        Variant7(StyleStmt),
        Variant8(ClassDefStmt),
        Variant9(ClassAssignStmt),
        Variant10(ClickStmt),
        Variant11(LinkStyleStmt),
        Variant12(usize),
        Variant13((Vec<Node>, Vec<Edge>)),
        Variant14(Vec<String>),
        Variant15(Option<String>),
        Variant16(Option<LabeledText>),
        Variant17((Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)),
        Variant18(alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>),
        Variant19(FlowchartAst),
        Variant20((String, Option<String>, SourceSpan)),
        Variant21(Vec<Node>),
        Variant22(Node),
        Variant23(alloc::vec::Vec<Node>),
        Variant24(()),
        Variant25(Stmt),
        Variant26(Vec<Stmt>),
        Variant27(SubgraphBlock),
        Variant28(Option<SubgraphHeader>),
    }
    const __ACTION: &[i8] = &[
        // State 0
        -42, -42, -42, -42, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 1
        36, 34, 35, 37, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 2
        -42, -42, -42, -42, -42, -42, 3, 0, 0, 0, -42, -42, 0, 0, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 3
        0, 0, 0, 0, -42, 0, 3, 0, 0, 0, -42, -42, 0, 0, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 4
        0, 0, 0, 0, 9, 0, 0, 0, 0, 0, 51, 10, 0, 0, 0, 0, 53, 49, 48, 50, 52, 0, 0,
        // State 5
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 55, 0,
        // State 6
        0, 0, 0, 0, 0, 0, 0, 14, 0, 0, 0, 0, 0, -27, 0, 0, 0, 0, 0, 0, 0, -27, 0,
        // State 7
        0, 0, 0, 0, -42, -42, 3, 0, 0, 0, -42, -42, 0, 0, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 8
        0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0,
        // State 9
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 19, -5, 0, 0, -5, -5, -5, -5, -5, -5, 20,
        // State 10
        0, 0, 0, 0, -3, -3, -3, 0, 0, 0, -3, -3, 0, 12, 0, 0, -3, -3, -3, -3, -3, 55, 0,
        // State 11
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 22, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 12
        0, 0, 0, 0, -29, -29, -29, 14, 0, 0, -29, -29, 0, -28, 0, 0, -29, -29, -29, -29, -29, -28, 0,
        // State 13
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 14
        0, 0, 0, 0, 9, -59, 0, 0, 0, 0, 51, 10, 0, 0, 0, 0, 53, 49, 48, 50, 52, 0, 0,
        // State 15
        0, 0, 0, 0, 9, -60, 0, 0, 0, 0, 51, 10, 0, 0, 0, 0, 53, 49, 48, 50, 52, 0, 0,
        // State 16
        0, 0, 0, 0, -42, -42, 3, 0, 0, 0, -42, -42, 0, 0, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 17
        0, 0, 0, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 18
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, -5, -5, 27,
        // State 19
        0, 0, 0, 0, -48, -48, -48, -5, 59, 0, -48, -48, 0, -5, 0, 0, -48, -48, -48, -48, -48, -5, 0,
        // State 20
        0, 0, 0, 0, -27, -27, -27, 14, 0, 0, -27, -27, 0, -27, 0, 0, -27, -27, -27, -27, -27, -27, 0,
        // State 21
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 22
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 29, -5, 0, 0, -5, -5, -5, -5, -5, -5, 30,
        // State 23
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 31, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 24
        0, 0, 0, 0, -42, -42, 3, 0, 0, 0, -42, -42, 0, 0, 0, 0, -42, -42, -42, -42, -42, 0, 0,
        // State 25
        0, 0, 0, 0, 9, -60, 0, 0, 0, 0, 51, 10, 0, 0, 0, 0, 53, 49, 48, 50, 52, 0, 0,
        // State 26
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, -5, -5, 0,
        // State 27
        0, 0, 0, 0, -28, -28, -28, 14, 0, 0, -28, -28, 0, -28, 0, 0, -28, -28, -28, -28, -28, -28, 0,
        // State 28
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, -5, -5, 32,
        // State 29
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, -5, -5, 0,
        // State 30
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 31
        0, 0, 0, 0, -5, -5, -5, -5, 59, 0, -5, -5, 0, -5, 0, 0, -5, -5, -5, -5, -5, -5, 0,
        // State 32
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 33
        0, 0, 0, 0, -22, 0, -22, 0, 0, 39, -22, -22, 0, 0, 0, 0, -22, -22, -22, -22, -22, 0, 0,
        // State 34
        0, 0, 0, 0, -24, 0, -24, 0, 0, 40, -24, -24, 0, 0, 0, 0, -24, -24, -24, -24, -24, 0, 0,
        // State 35
        0, 0, 0, 0, -20, 0, -20, 0, 0, 41, -20, -20, 0, 0, 0, 0, -20, -20, -20, -20, -20, 0, 0,
        // State 36
        0, 0, 0, 0, -26, 0, -26, 0, 0, 42, -26, -26, 0, 0, 0, 0, -26, -26, -26, -26, -26, 0, 0,
        // State 37
        -43, -43, -43, -43, -43, -43, 0, 0, 0, 0, -43, -43, 0, 0, 0, 0, -43, -43, -43, -43, -43, 0, 0,
        // State 38
        0, 0, 0, 0, -21, 0, -21, 0, 0, 0, -21, -21, 0, 0, 0, 0, -21, -21, -21, -21, -21, 0, 0,
        // State 39
        0, 0, 0, 0, -23, 0, -23, 0, 0, 0, -23, -23, 0, 0, 0, 0, -23, -23, -23, -23, -23, 0, 0,
        // State 40
        0, 0, 0, 0, -19, 0, -19, 0, 0, 0, -19, -19, 0, 0, 0, 0, -19, -19, -19, -19, -19, 0, 0,
        // State 41
        0, 0, 0, 0, -25, 0, -25, 0, 0, 0, -25, -25, 0, 0, 0, 0, -25, -25, -25, -25, -25, 0, 0,
        // State 42
        0, 0, 0, 0, -47, -47, -47, 0, 0, 0, -47, -47, 0, 0, 0, 0, -47, -47, -47, -47, -47, 0, 0,
        // State 43
        0, 0, 0, 0, -49, -49, -49, 0, 0, 0, -49, -49, 0, 0, 0, 0, -49, -49, -49, -49, -49, 0, 0,
        // State 44
        0, 0, 0, 0, -50, -50, -50, 0, 0, 0, -50, -50, 0, 0, 0, 0, -50, -50, -50, -50, -50, 0, 0,
        // State 45
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 46
        0, 0, 0, 0, -51, -51, -51, 0, 0, 0, -51, -51, 0, 0, 0, 0, -51, -51, -51, -51, -51, 0, 0,
        // State 47
        0, 0, 0, 0, -55, -55, -55, 0, 0, 0, -55, -55, 0, 0, 0, 0, -55, -55, -55, -55, -55, 0, 0,
        // State 48
        0, 0, 0, 0, -54, -54, -54, 0, 0, 0, -54, -54, 0, 0, 0, 0, -54, -54, -54, -54, -54, 0, 0,
        // State 49
        0, 0, 0, 0, -56, -56, -56, 0, 0, 0, -56, -56, 0, 0, 0, 0, -56, -56, -56, -56, -56, 0, 0,
        // State 50
        0, 0, 0, 0, -52, -52, -52, 0, 0, 0, -52, -52, 0, 0, 0, 0, -52, -52, -52, -52, -52, 0, 0,
        // State 51
        0, 0, 0, 0, -57, -57, -57, 0, 0, 0, -57, -57, 0, 0, 0, 0, -57, -57, -57, -57, -57, 0, 0,
        // State 52
        0, 0, 0, 0, -53, -53, -53, 0, 0, 0, -53, -53, 0, 0, 0, 0, -53, -53, -53, -53, -53, 0, 0,
        // State 53
        0, 0, 0, 0, -16, -16, -16, 0, 0, 0, -16, -16, 0, -16, 0, 0, -16, -16, -16, -16, -16, -16, 0,
        // State 54
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 55
        0, 0, 0, 0, -33, -33, -33, -33, 0, 0, -33, -33, 0, -33, 0, 0, -33, -33, -33, -33, -33, -33, 0,
        // State 56
        0, 0, 0, 0, 0, -61, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 57
        0, 0, 0, 0, -37, -37, -37, -41, 0, 0, -37, -37, 0, -41, 0, 0, -37, -37, -37, -37, -37, -41, 0,
        // State 58
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 68, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 59
        0, 0, 0, 0, -17, -17, -17, 0, 0, 0, -17, -17, 0, -17, 0, 0, -17, -17, -17, -17, -17, -17, 0,
        // State 60
        0, 0, 0, 0, -15, -15, -15, 0, 0, 0, -15, -15, 0, -15, 0, 0, -15, -15, -15, -15, -15, -15, 0,
        // State 61
        0, 0, 0, 0, -34, -34, -34, -34, 0, 0, -34, -34, 0, -34, 0, 0, -34, -34, -34, -34, -34, -34, 0,
        // State 62
        0, 0, 0, 0, -30, -30, -30, -30, 0, 0, -30, -30, 0, -30, 0, 0, -30, -30, -30, -30, -30, -30, 0,
        // State 63
        0, 0, 0, 0, 0, 73, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 64
        0, 0, 0, 0, -44, -44, 0, 0, 0, 0, -44, -44, 0, 0, 0, 0, -44, -44, -44, -44, -44, 0, 0,
        // State 65
        0, 0, 0, 0, -36, -36, -36, -39, 0, 0, -36, -36, 0, -39, 0, 0, -36, -36, -36, -36, -36, -39, 0,
        // State 66
        0, 0, 0, 0, -40, -40, -40, -40, 0, 0, -40, -40, 0, -40, 0, 0, -40, -40, -40, -40, -40, -40, 0,
        // State 67
        0, 0, 0, 0, -4, -4, -4, -4, 0, 0, -4, -4, 0, -4, 0, 0, -4, -4, -4, -4, -4, -4, 0,
        // State 68
        0, 0, 0, 0, -14, -14, -14, 0, 0, 0, -14, -14, 0, -14, 0, 0, -14, -14, -14, -14, -14, -14, 0,
        // State 69
        0, 0, 0, 0, -41, -41, -41, -41, 0, 0, -41, -41, 0, -41, 0, 0, -41, -41, -41, -41, -41, -41, 0,
        // State 70
        0, 0, 0, 0, -13, -13, -13, 0, 0, 0, -13, -13, 0, -13, 0, 0, -13, -13, -13, -13, -13, -13, 0,
        // State 71
        0, 0, 0, 0, 0, -58, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 72
        0, 0, 0, 0, -63, -63, -63, 0, 0, 0, -63, -63, 0, 0, 0, 0, -63, -63, -63, -63, -63, 0, 0,
        // State 73
        0, 0, 0, 0, 0, 78, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        // State 74
        0, 0, 0, 0, -35, -35, -35, -38, 0, 0, -35, -35, 0, -38, 0, 0, -35, -35, -35, -35, -35, -38, 0,
        // State 75
        0, 0, 0, 0, -39, -39, -39, -39, 0, 0, -39, -39, 0, -39, 0, 0, -39, -39, -39, -39, -39, -39, 0,
        // State 76
        0, 0, 0, 0, -12, -12, -12, 0, 0, 0, -12, -12, 0, -12, 0, 0, -12, -12, -12, -12, -12, -12, 0,
        // State 77
        0, 0, 0, 0, -62, -62, -62, 0, 0, 0, -62, -62, 0, 0, 0, 0, -62, -62, -62, -62, -62, 0, 0,
        // State 78
        0, 0, 0, 0, -38, -38, -38, -38, 0, 0, -38, -38, 0, -38, 0, 0, -38, -38, -38, -38, -38, -38, 0,
    ];
    fn __action(state: i8, integer: usize) -> i8 {
        __ACTION[(state as usize) * 23 + integer]
    }
    const __EOF_ACTION: &[i8] = &[
        // State 0
        0,
        // State 1
        0,
        // State 2
        -42,
        // State 3
        -42,
        // State 4
        -60,
        // State 5
        0,
        // State 6
        0,
        // State 7
        -42,
        // State 8
        0,
        // State 9
        -5,
        // State 10
        -3,
        // State 11
        0,
        // State 12
        -29,
        // State 13
        0,
        // State 14
        -59,
        // State 15
        0,
        // State 16
        0,
        // State 17
        0,
        // State 18
        -5,
        // State 19
        -48,
        // State 20
        -27,
        // State 21
        0,
        // State 22
        -5,
        // State 23
        0,
        // State 24
        -42,
        // State 25
        0,
        // State 26
        -5,
        // State 27
        -28,
        // State 28
        -5,
        // State 29
        -5,
        // State 30
        0,
        // State 31
        -5,
        // State 32
        -66,
        // State 33
        -22,
        // State 34
        -24,
        // State 35
        -20,
        // State 36
        -26,
        // State 37
        -43,
        // State 38
        -21,
        // State 39
        -23,
        // State 40
        -19,
        // State 41
        -25,
        // State 42
        -47,
        // State 43
        -49,
        // State 44
        -50,
        // State 45
        -18,
        // State 46
        -51,
        // State 47
        -55,
        // State 48
        -54,
        // State 49
        -56,
        // State 50
        -52,
        // State 51
        -57,
        // State 52
        -53,
        // State 53
        -16,
        // State 54
        0,
        // State 55
        -33,
        // State 56
        -61,
        // State 57
        -37,
        // State 58
        0,
        // State 59
        -17,
        // State 60
        -15,
        // State 61
        -34,
        // State 62
        -30,
        // State 63
        0,
        // State 64
        0,
        // State 65
        -36,
        // State 66
        -40,
        // State 67
        -4,
        // State 68
        -14,
        // State 69
        -41,
        // State 70
        -13,
        // State 71
        -58,
        // State 72
        -63,
        // State 73
        0,
        // State 74
        -35,
        // State 75
        -39,
        // State 76
        -12,
        // State 77
        -62,
        // State 78
        -38,
    ];
    fn __goto(state: i8, nt: usize) -> i8 {
        match nt {
            2 => 42,
            3 => match state {
                9 => 57,
                18 => 65,
                22 => 69,
                26 => 74,
                28 => 75,
                31 => 78,
                _ => 66,
            },
            7 => match state {
                10 => 59,
                _ => 53,
            },
            8 => 10,
            9 => 32,
            10 => 3,
            11 => match state {
                11 => 60,
                21 => 68,
                23 => 70,
                30 => 76,
                _ => 5,
            },
            12 => 43,
            13 => match state {
                12 | 27 => 61,
                _ => 55,
            },
            15 => match state {
                20 => 27,
                _ => 12,
            },
            16 => 44,
            17 => match state {
                4 | 14..=15 | 25 => 6,
                13 => 62,
                _ => 20,
            },
            18 => match state {
                0 => 1,
                3 => 4,
                2 => 37,
                16 => 64,
                _ => 14,
            },
            19 => match state {
                17 => 25,
                _ => 15,
            },
            21 => match state {
                14 => 24,
                _ => 7,
            },
            22 => match state {
                24 => 71,
                _ => 56,
            },
            23 => match state {
                15 => 63,
                25 => 73,
                _ => 45,
            },
            24 => 46,
            _ => 0,
        }
    }
    #[allow(clippy::needless_raw_string_hashes)]
    const __TERMINAL: &[&str] = &[
        r###""graph""###,
        r###""flowchart""###,
        r###""flowchart-elk""###,
        r###""swimlane-beta""###,
        r###""subgraph""###,
        r###""end""###,
        r###"Sep"###,
        r###"Amp"###,
        r###"StyleSep"###,
        r###"Direction"###,
        r###"DirectionStmt"###,
        r###"Id"###,
        r###"NodeLabel"###,
        r###"Arrow"###,
        r###"EdgeLabel"###,
        r###"SubgraphHeader"###,
        r###"StyleStmt"###,
        r###"ClassDefStmt"###,
        r###"ClassAssignStmt"###,
        r###"ClickStmt"###,
        r###"LinkStyleStmt"###,
        r###"EdgeId"###,
        r###"ShapeData"###,
    ];
    fn __expected_tokens(__state: i8) -> alloc::vec::Vec<alloc::string::String> {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            let next_state = __action(__state, index);
            if next_state == 0 {
                None
            } else {
                Some(alloc::string::ToString::to_string(terminal))
            }
        }).collect()
    }
    fn __expected_tokens_from_states<
    >(
        __states: &[i8],
        _: core::marker::PhantomData<()>,
    ) -> alloc::vec::Vec<alloc::string::String>
    {
        __TERMINAL.iter().enumerate().filter_map(|(index, terminal)| {
            if __accepts(None, __states, Some(index), core::marker::PhantomData::<()>) {
                Some(alloc::string::ToString::to_string(terminal))
            } else {
                None
            }
        }).collect()
    }
    struct __StateMachine<>
    where
    {
        __phantom: core::marker::PhantomData<()>,
    }
    impl<> __state_machine::ParserDefinition for __StateMachine<>
    where
    {
        type Location = usize;
        type Error = crate::diagrams::flowchart::LexError;
        type Token = Tok;
        type TokenIndex = usize;
        type Symbol = __Symbol<>;
        type Success = FlowchartAst;
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
            __token_to_integer(token, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn action(&self, state: i8, integer: usize) -> i8 {
            __action(state, integer)
        }

        #[inline]
        fn error_action(&self, state: i8) -> i8 {
            __action(state, 23 - 1)
        }

        #[inline]
        fn eof_action(&self, state: i8) -> i8 {
            __EOF_ACTION[state as usize]
        }

        #[inline]
        fn goto(&self, state: i8, nt: usize) -> i8 {
            __goto(state, nt)
        }

        fn token_to_symbol(&self, token_index: usize, token: Self::Token) -> Self::Symbol {
            __token_to_symbol(token_index, token, core::marker::PhantomData::<()>)
        }

        fn expected_tokens(&self, state: i8) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens(state)
        }

        fn expected_tokens_from_states(&self, states: &[i8]) -> alloc::vec::Vec<alloc::string::String> {
            __expected_tokens_from_states(states, core::marker::PhantomData::<()>)
        }

        #[inline]
        fn uses_error_recovery(&self) -> bool {
            false
        }

        #[inline]
        fn error_recovery_symbol(
            &self,
            recovery: __state_machine::ErrorRecovery<Self>,
        ) -> Self::Symbol {
            panic!("error recovery not enabled for this grammar")
        }

        fn reduce(
            &mut self,
            action: i8,
            start_location: Option<&Self::Location>,
            states: &mut alloc::vec::Vec<i8>,
            symbols: &mut alloc::vec::Vec<__state_machine::SymbolTriple<Self>>,
        ) -> Option<__state_machine::ParseResult<Self>> {
            __reduce(
                action,
                start_location,
                states,
                symbols,
                core::marker::PhantomData::<()>,
            )
        }

        fn simulate_reduce(&self, action: i8) -> __state_machine::SimulatedReduce<Self> {
            __simulate_reduce(action, core::marker::PhantomData::<()>)
        }
    }
    fn __token_to_integer<
    >(
        __token: &Tok,
        _: core::marker::PhantomData<()>,
    ) -> Option<usize>
    {
        #[warn(unused_variables)]
        match __token {
            Tok::KwGraph if true => Some(0),
            Tok::KwFlowchart if true => Some(1),
            Tok::KwFlowchartElk if true => Some(2),
            Tok::KwSwimlane if true => Some(3),
            Tok::KwSubgraph if true => Some(4),
            Tok::KwEnd if true => Some(5),
            Tok::Sep if true => Some(6),
            Tok::Amp if true => Some(7),
            Tok::StyleSep if true => Some(8),
            Tok::Direction(_) if true => Some(9),
            Tok::DirectionStmt(_) if true => Some(10),
            Tok::Id(_) if true => Some(11),
            Tok::NodeLabel(_) if true => Some(12),
            Tok::Arrow(_) if true => Some(13),
            Tok::EdgeLabel(_) if true => Some(14),
            Tok::SubgraphHeader(_) if true => Some(15),
            Tok::StyleStmt(_) if true => Some(16),
            Tok::ClassDefStmt(_) if true => Some(17),
            Tok::ClassAssignStmt(_) if true => Some(18),
            Tok::ClickStmt(_) if true => Some(19),
            Tok::LinkStyleStmt(_) if true => Some(20),
            Tok::EdgeId(_) if true => Some(21),
            Tok::ShapeData(_) if true => Some(22),
            _ => None,
        }
    }
    fn __token_to_symbol<
    >(
        __token_index: usize,
        __token: Tok,
        _: core::marker::PhantomData<()>,
    ) -> __Symbol<>
    {
        #[allow(clippy::manual_range_patterns)]match __token_index {
            0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 => __Symbol::Variant0(__token),
            9 | 11 | 21 | 22 => match __token {
                Tok::Direction(__tok0) | Tok::Id(__tok0) | Tok::EdgeId(__tok0) | Tok::ShapeData(__tok0) if true => __Symbol::Variant1(__tok0),
                _ => unreachable!(),
            },
            10 => match __token {
                Tok::DirectionStmt(__tok0) if true => __Symbol::Variant2(__tok0),
                _ => unreachable!(),
            },
            12 => match __token {
                Tok::NodeLabel(__tok0) if true => __Symbol::Variant3(__tok0),
                _ => unreachable!(),
            },
            13 => match __token {
                Tok::Arrow(__tok0) if true => __Symbol::Variant4(__tok0),
                _ => unreachable!(),
            },
            14 => match __token {
                Tok::EdgeLabel(__tok0) if true => __Symbol::Variant5(__tok0),
                _ => unreachable!(),
            },
            15 => match __token {
                Tok::SubgraphHeader(__tok0) if true => __Symbol::Variant6(__tok0),
                _ => unreachable!(),
            },
            16 => match __token {
                Tok::StyleStmt(__tok0) if true => __Symbol::Variant7(__tok0),
                _ => unreachable!(),
            },
            17 => match __token {
                Tok::ClassDefStmt(__tok0) if true => __Symbol::Variant8(__tok0),
                _ => unreachable!(),
            },
            18 => match __token {
                Tok::ClassAssignStmt(__tok0) if true => __Symbol::Variant9(__tok0),
                _ => unreachable!(),
            },
            19 => match __token {
                Tok::ClickStmt(__tok0) if true => __Symbol::Variant10(__tok0),
                _ => unreachable!(),
            },
            20 => match __token {
                Tok::LinkStyleStmt(__tok0) if true => __Symbol::Variant11(__tok0),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
    fn __simulate_reduce<
    >(
        __reduce_index: i8,
        _: core::marker::PhantomData<()>,
    ) -> __state_machine::SimulatedReduce<__StateMachine<>>
    {
        match __reduce_index {
            0 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 0,
                }
            }
            1 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 1,
                }
            }
            2 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 2,
                }
            }
            3 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 3,
                }
            }
            4 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 3,
                }
            }
            5 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 4,
                }
            }
            6 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 4,
                }
            }
            7 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 5,
                }
            }
            8 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 5,
                }
            }
            9 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 6,
                }
            }
            10 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 6,
                }
            }
            11 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 7,
                }
            }
            12 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 7,
                }
            }
            13 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 7,
                }
            }
            14 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 7,
                }
            }
            15 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 8,
                }
            }
            16 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 8,
                }
            }
            17 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 9,
                }
            }
            18 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 10,
                }
            }
            19 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            20 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 10,
                }
            }
            21 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            22 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 10,
                }
            }
            23 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            24 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 10,
                }
            }
            25 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 10,
                }
            }
            26 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 11,
                }
            }
            27 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 11,
                }
            }
            28 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 12,
                }
            }
            29 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 13,
                }
            }
            30 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 14,
                }
            }
            31 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 14,
                }
            }
            32 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 15,
                }
            }
            33 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 15,
                }
            }
            34 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 16,
                }
            }
            35 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 16,
                }
            }
            36 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 16,
                }
            }
            37 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 17,
                }
            }
            38 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 17,
                }
            }
            39 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 17,
                }
            }
            40 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 17,
                }
            }
            41 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 18,
                }
            }
            42 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 18,
                }
            }
            43 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 19,
                }
            }
            44 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 20,
                }
            }
            45 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 20,
                }
            }
            46 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            47 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 21,
                }
            }
            48 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            49 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            50 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            51 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            52 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            53 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            54 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            55 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            56 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 21,
                }
            }
            57 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 3,
                    nonterminal_produced: 22,
                }
            }
            58 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 22,
                }
            }
            59 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 23,
                }
            }
            60 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 2,
                    nonterminal_produced: 23,
                }
            }
            61 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 5,
                    nonterminal_produced: 24,
                }
            }
            62 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 4,
                    nonterminal_produced: 24,
                }
            }
            63 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 1,
                    nonterminal_produced: 25,
                }
            }
            64 => {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop: 0,
                    nonterminal_produced: 25,
                }
            }
            65 => __state_machine::SimulatedReduce::Accept,
            _ => panic!("invalid reduction index {__reduce_index}")
        }
    }
    pub struct FlowchartAstParser {
        _priv: (),
    }

    impl Default for FlowchartAstParser { fn default() -> Self { Self::new() } }
    impl FlowchartAstParser {
        pub fn new() -> FlowchartAstParser {
            FlowchartAstParser {
                _priv: (),
            }
        }

        #[allow(dead_code)]
        pub fn parse<
            __TOKEN: __ToTriple<>,
            __TOKENS: IntoIterator<Item=__TOKEN>,
        >(
            &self,
            __tokens0: __TOKENS,
        ) -> Result<FlowchartAst, __lalrpop_util::ParseError<usize, Tok, crate::diagrams::flowchart::LexError>>
        {
            let __tokens = __tokens0.into_iter();
            let mut __tokens = __tokens.map(|t| __ToTriple::to_triple(t));
            __state_machine::Parser::drive(
                __StateMachine {
                    __phantom: core::marker::PhantomData::<()>,
                },
                __tokens,
            )
        }
    }
    fn __accepts<
    >(
        __error_state: Option<i8>,
        __states: &[i8],
        __opt_integer: Option<usize>,
        _: core::marker::PhantomData<()>,
    ) -> bool
    {
        let mut __states = __states.to_vec();
        __states.extend(__error_state);
        loop {
            let mut __states_len = __states.len();
            let __top = __states[__states_len - 1];
            let __action = match __opt_integer {
                None => __EOF_ACTION[__top as usize],
                Some(__integer) => __action(__top, __integer),
            };
            if __action == 0 { return false; }
            if __action > 0 { return true; }
            let (__to_pop, __nt) = match __simulate_reduce(-(__action + 1), core::marker::PhantomData::<()>) {
                __state_machine::SimulatedReduce::Reduce {
                    states_to_pop, nonterminal_produced
                } => (states_to_pop, nonterminal_produced),
                __state_machine::SimulatedReduce::Accept => return true,
            };
            __states_len -= __to_pop;
            __states.truncate(__states_len);
            let __top = __states[__states_len - 1];
            let __next_state = __goto(__top, __nt);
            __states.push(__next_state);
        }
    }
    fn __reduce<
    >(
        __action: i8,
        __lookahead_start: Option<&usize>,
        __states: &mut alloc::vec::Vec<i8>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> Option<Result<FlowchartAst,__lalrpop_util::ParseError<usize, Tok, crate::diagrams::flowchart::LexError>>>
    {
        let (__pop_states, __nonterminal) = match __action {
            0 => {
                __reduce0(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            1 => {
                __reduce1(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            2 => {
                __reduce2(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            3 => {
                __reduce3(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            4 => {
                __reduce4(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            5 => {
                __reduce5(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            6 => {
                __reduce6(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            7 => {
                __reduce7(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            8 => {
                __reduce8(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            9 => {
                __reduce9(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            10 => {
                __reduce10(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            11 => {
                __reduce11(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            12 => {
                __reduce12(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            13 => {
                __reduce13(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            14 => {
                __reduce14(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            15 => {
                __reduce15(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            16 => {
                __reduce16(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            17 => {
                __reduce17(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            18 => {
                __reduce18(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            19 => {
                __reduce19(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            20 => {
                __reduce20(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            21 => {
                __reduce21(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            22 => {
                __reduce22(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            23 => {
                __reduce23(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            24 => {
                __reduce24(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            25 => {
                __reduce25(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            26 => {
                __reduce26(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            27 => {
                __reduce27(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            28 => {
                __reduce28(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            29 => {
                __reduce29(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            30 => {
                __reduce30(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            31 => {
                __reduce31(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            32 => {
                __reduce32(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            33 => {
                __reduce33(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            34 => {
                __reduce34(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            35 => {
                __reduce35(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            36 => {
                __reduce36(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            37 => {
                __reduce37(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            38 => {
                __reduce38(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            39 => {
                __reduce39(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            40 => {
                __reduce40(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            41 => {
                __reduce41(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            42 => {
                __reduce42(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            43 => {
                __reduce43(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            44 => {
                __reduce44(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            45 => {
                __reduce45(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            46 => {
                __reduce46(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            47 => {
                __reduce47(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            48 => {
                __reduce48(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            49 => {
                __reduce49(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            50 => {
                __reduce50(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            51 => {
                __reduce51(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            52 => {
                __reduce52(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            53 => {
                __reduce53(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            54 => {
                __reduce54(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            55 => {
                __reduce55(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            56 => {
                __reduce56(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            57 => {
                __reduce57(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            58 => {
                __reduce58(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            59 => {
                __reduce59(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            60 => {
                __reduce60(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            61 => {
                __reduce61(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            62 => {
                __reduce62(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            63 => {
                __reduce63(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            64 => {
                __reduce64(__lookahead_start, __symbols, core::marker::PhantomData::<()>)
            }
            65 => {
                // __FlowchartAst = FlowchartAst => ActionFn(0);
                let __sym0 = __pop_Variant19(__symbols);
                let __start = __sym0.0.clone();
                let __end = __sym0.2.clone();
                let __nt = super::__action0::<>(__sym0);
                return Some(Ok(__nt));
            }
            _ => panic!("invalid action code {__action}")
        };
        let __states_len = __states.len();
        __states.truncate(__states_len - __pop_states);
        let __state = *__states.last().unwrap();
        let __next_state = __goto(__state, __nonterminal);
        __states.push(__next_state);
        None
    }
    #[inline(never)]
    fn __symbol_type_mismatch() -> ! {
        panic!("symbol type mismatch")
    }
    fn __pop_Variant24<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, (), usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant24(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant17<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>), usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant17(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant20<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, (String, Option<String>, SourceSpan), usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant20(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant13<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, (Vec<Node>, Vec<Edge>), usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant13(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant4<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, ArrowToken, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant4(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant9<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, ClassAssignStmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant9(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant8<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, ClassDefStmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant8(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant10<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, ClickStmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant10(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant2<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, DirectionStatementToken, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant2(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant19<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, FlowchartAst, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant19(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant5<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, LabeledText, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant5(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant11<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, LinkStyleStmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant11(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant22<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Node, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant22(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant3<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, NodeLabelToken, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant3(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant16<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Option<LabeledText>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant16(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant15<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Option<String>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant15(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant28<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Option<SubgraphHeader>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant28(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant25<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Stmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant25(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant1<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, String, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant1(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant7<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, StyleStmt, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant7(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant27<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, SubgraphBlock, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant27(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant6<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, SubgraphHeader, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant6(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant0<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Tok, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant0(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant21<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<Node>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant21(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant26<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<Stmt>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant26(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant14<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, Vec<String>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant14(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant18<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant18(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant23<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, alloc::vec::Vec<Node>, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant23(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __pop_Variant12<
    >(
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>
    ) -> (usize, usize, usize)
     {
        match __symbols.pop() {
            Some((__l, __Symbol::Variant12(__v), __r)) => (__l, __v, __r),
            _ => __symbol_type_mismatch()
        }
    }
    fn __reduce0<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // @L =  => ActionFn(54);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action54::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant12(__nt), __end));
        (0, 0)
    }
    fn __reduce1<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // @R =  => ActionFn(51);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action51::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant12(__nt), __end));
        (0, 1)
    }
    fn __reduce2<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Chain = NodeGroup, EdgeSeg+ => ActionFn(23);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant18(__symbols);
        let __sym0 = __pop_Variant21(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action23::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant13(__nt), __end));
        (2, 2)
    }
    fn __reduce3<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ClassOpt = StyleSep, Id => ActionFn(35);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action35::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (2, 3)
    }
    fn __reduce4<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ClassOpt =  => ActionFn(36);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action36::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant14(__nt), __end));
        (0, 3)
    }
    fn __reduce5<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Direction? = Direction => ActionFn(52);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action52::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (1, 4)
    }
    fn __reduce6<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Direction? =  => ActionFn(53);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action53::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (0, 4)
    }
    fn __reduce7<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeId? = EdgeId => ActionFn(45);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action45::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (1, 5)
    }
    fn __reduce8<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeId? =  => ActionFn(46);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action46::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (0, 5)
    }
    fn __reduce9<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeLabel? = EdgeLabel => ActionFn(43);
        let __sym0 = __pop_Variant5(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action43::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant16(__nt), __end));
        (1, 6)
    }
    fn __reduce10<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeLabel? =  => ActionFn(44);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action44::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant16(__nt), __end));
        (0, 6)
    }
    fn __reduce11<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg = EdgeId, Arrow, EdgeLabel, NodeGroup => ActionFn(85);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant21(__symbols);
        let __sym2 = __pop_Variant5(__symbols);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym3.2.clone();
        let __nt = super::__action85::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (4, 7)
    }
    fn __reduce12<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg = EdgeId, Arrow, NodeGroup => ActionFn(86);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant21(__symbols);
        let __sym1 = __pop_Variant4(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action86::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (3, 7)
    }
    fn __reduce13<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg = Arrow, EdgeLabel, NodeGroup => ActionFn(87);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant21(__symbols);
        let __sym1 = __pop_Variant5(__symbols);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action87::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (3, 7)
    }
    fn __reduce14<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg = Arrow, NodeGroup => ActionFn(88);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant21(__symbols);
        let __sym0 = __pop_Variant4(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action88::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant17(__nt), __end));
        (2, 7)
    }
    fn __reduce15<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg+ = EdgeSeg => ActionFn(49);
        let __sym0 = __pop_Variant17(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action49::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant18(__nt), __end));
        (1, 8)
    }
    fn __reduce16<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // EdgeSeg+ = EdgeSeg+, EdgeSeg => ActionFn(50);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant17(__symbols);
        let __sym0 = __pop_Variant18(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action50::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant18(__nt), __end));
        (2, 8)
    }
    fn __reduce17<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // FlowchartAst = Seps, Header, Seps, Statements => ActionFn(1);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant26(__symbols);
        let __sym2 = __pop_Variant24(__symbols);
        let __sym1 = __pop_Variant20(__symbols);
        let __sym0 = __pop_Variant24(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym3.2.clone();
        let __nt = super::__action1::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant19(__nt), __end));
        (4, 9)
    }
    fn __reduce18<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "graph", Direction => ActionFn(75);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action75::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (2, 10)
    }
    fn __reduce19<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "graph" => ActionFn(76);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action76::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (1, 10)
    }
    fn __reduce20<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "flowchart", Direction => ActionFn(77);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action77::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (2, 10)
    }
    fn __reduce21<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "flowchart" => ActionFn(78);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action78::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (1, 10)
    }
    fn __reduce22<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "flowchart-elk", Direction => ActionFn(79);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action79::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (2, 10)
    }
    fn __reduce23<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "flowchart-elk" => ActionFn(80);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action80::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (1, 10)
    }
    fn __reduce24<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "swimlane-beta", Direction => ActionFn(81);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action81::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (2, 10)
    }
    fn __reduce25<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Header = "swimlane-beta" => ActionFn(82);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action82::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant20(__nt), __end));
        (1, 10)
    }
    fn __reduce26<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroup = NodeRefChain => ActionFn(89);
        let __sym0 = __pop_Variant22(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action89::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant21(__nt), __end));
        (1, 11)
    }
    fn __reduce27<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroup = NodeRefChain, NodeGroupRest+ => ActionFn(90);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant23(__symbols);
        let __sym0 = __pop_Variant22(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action90::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant21(__nt), __end));
        (2, 11)
    }
    fn __reduce28<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupOnly = NodeRefChain, NodeGroupRest+ => ActionFn(28);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant23(__symbols);
        let __sym0 = __pop_Variant22(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action28::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant21(__nt), __end));
        (2, 12)
    }
    fn __reduce29<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupRest = Amp, NodeRefChain => ActionFn(29);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant22(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action29::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (2, 13)
    }
    fn __reduce30<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupRest* =  => ActionFn(41);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action41::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant23(__nt), __end));
        (0, 14)
    }
    fn __reduce31<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupRest* = NodeGroupRest+ => ActionFn(42);
        let __sym0 = __pop_Variant23(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action42::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant23(__nt), __end));
        (1, 14)
    }
    fn __reduce32<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupRest+ = NodeGroupRest => ActionFn(39);
        let __sym0 = __pop_Variant22(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action39::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant23(__nt), __end));
        (1, 15)
    }
    fn __reduce33<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeGroupRest+ = NodeGroupRest+, NodeGroupRest => ActionFn(40);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant22(__symbols);
        let __sym0 = __pop_Variant23(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action40::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant23(__nt), __end));
        (2, 15)
    }
    fn __reduce34<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRef = Id, NodeLabel, ShapeData, ClassOpt => ActionFn(91);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant14(__symbols);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym3.2.clone();
        let __nt = super::__action91::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (4, 16)
    }
    fn __reduce35<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRef = Id, NodeLabel, ClassOpt => ActionFn(92);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant14(__symbols);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action92::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (3, 16)
    }
    fn __reduce36<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRef = Id, ClassOpt => ActionFn(70);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant14(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action70::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (2, 16)
    }
    fn __reduce37<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRefChain = Id, NodeLabel, ShapeData, ClassOpt => ActionFn(93);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant14(__symbols);
        let __sym2 = __pop_Variant1(__symbols);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym3.2.clone();
        let __nt = super::__action93::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (4, 17)
    }
    fn __reduce38<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRefChain = Id, NodeLabel, ClassOpt => ActionFn(94);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant14(__symbols);
        let __sym1 = __pop_Variant3(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action94::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (3, 17)
    }
    fn __reduce39<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRefChain = Id, ShapeData, ClassOpt => ActionFn(72);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant14(__symbols);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action72::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (3, 17)
    }
    fn __reduce40<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // NodeRefChain = Id, ClassOpt => ActionFn(73);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant14(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action73::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant22(__nt), __end));
        (2, 17)
    }
    fn __reduce41<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Seps =  => ActionFn(10);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action10::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant24(__nt), __end));
        (0, 18)
    }
    fn __reduce42<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Seps = Sep, Seps => ActionFn(11);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant24(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action11::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant24(__nt), __end));
        (2, 18)
    }
    fn __reduce43<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Seps1 = Sep, Seps => ActionFn(24);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant24(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action24::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant24(__nt), __end));
        (2, 19)
    }
    fn __reduce44<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ShapeData? = ShapeData => ActionFn(37);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action37::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (1, 20)
    }
    fn __reduce45<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // ShapeData? =  => ActionFn(38);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action38::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant15(__nt), __end));
        (0, 20)
    }
    fn __reduce46<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Chain => ActionFn(12);
        let __sym0 = __pop_Variant13(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action12::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce47<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = Id, ShapeData => ActionFn(74);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant1(__symbols);
        let __sym0 = __pop_Variant1(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action74::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (2, 21)
    }
    fn __reduce48<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = NodeGroupOnly => ActionFn(14);
        let __sym0 = __pop_Variant21(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action14::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce49<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = NodeRef => ActionFn(15);
        let __sym0 = __pop_Variant22(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action15::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce50<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = SubgraphBlock => ActionFn(16);
        let __sym0 = __pop_Variant27(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action16::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce51<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = DirectionStmt => ActionFn(17);
        let __sym0 = __pop_Variant2(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action17::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce52<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = StyleStmt => ActionFn(18);
        let __sym0 = __pop_Variant7(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action18::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce53<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ClassDefStmt => ActionFn(19);
        let __sym0 = __pop_Variant8(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action19::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce54<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ClassAssignStmt => ActionFn(20);
        let __sym0 = __pop_Variant9(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action20::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce55<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = ClickStmt => ActionFn(21);
        let __sym0 = __pop_Variant10(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action21::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce56<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statement = LinkStyleStmt => ActionFn(22);
        let __sym0 = __pop_Variant11(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action22::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant25(__nt), __end));
        (1, 21)
    }
    fn __reduce57<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StatementRest = Seps, Statement, StatementRest => ActionFn(8);
        assert!(__symbols.len() >= 3);
        let __sym2 = __pop_Variant26(__symbols);
        let __sym1 = __pop_Variant25(__symbols);
        let __sym0 = __pop_Variant24(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym2.2.clone();
        let __nt = super::__action8::<>(__sym0, __sym1, __sym2);
        __symbols.push((__start, __Symbol::Variant26(__nt), __end));
        (3, 22)
    }
    fn __reduce58<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // StatementRest = Seps => ActionFn(9);
        let __sym0 = __pop_Variant24(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action9::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant26(__nt), __end));
        (1, 22)
    }
    fn __reduce59<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements =  => ActionFn(6);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action6::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant26(__nt), __end));
        (0, 23)
    }
    fn __reduce60<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // Statements = Statement, StatementRest => ActionFn(7);
        assert!(__symbols.len() >= 2);
        let __sym1 = __pop_Variant26(__symbols);
        let __sym0 = __pop_Variant25(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym1.2.clone();
        let __nt = super::__action7::<>(__sym0, __sym1);
        __symbols.push((__start, __Symbol::Variant26(__nt), __end));
        (2, 23)
    }
    fn __reduce61<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // SubgraphBlock = "subgraph", SubgraphHeader, Seps1, Statements, "end" => ActionFn(95);
        assert!(__symbols.len() >= 5);
        let __sym4 = __pop_Variant0(__symbols);
        let __sym3 = __pop_Variant26(__symbols);
        let __sym2 = __pop_Variant24(__symbols);
        let __sym1 = __pop_Variant6(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym4.2.clone();
        let __nt = super::__action95::<>(__sym0, __sym1, __sym2, __sym3, __sym4);
        __symbols.push((__start, __Symbol::Variant27(__nt), __end));
        (5, 24)
    }
    fn __reduce62<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // SubgraphBlock = "subgraph", Seps1, Statements, "end" => ActionFn(96);
        assert!(__symbols.len() >= 4);
        let __sym3 = __pop_Variant0(__symbols);
        let __sym2 = __pop_Variant26(__symbols);
        let __sym1 = __pop_Variant24(__symbols);
        let __sym0 = __pop_Variant0(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym3.2.clone();
        let __nt = super::__action96::<>(__sym0, __sym1, __sym2, __sym3);
        __symbols.push((__start, __Symbol::Variant27(__nt), __end));
        (4, 24)
    }
    fn __reduce63<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // SubgraphHeader? = SubgraphHeader => ActionFn(47);
        let __sym0 = __pop_Variant6(__symbols);
        let __start = __sym0.0.clone();
        let __end = __sym0.2.clone();
        let __nt = super::__action47::<>(__sym0);
        __symbols.push((__start, __Symbol::Variant28(__nt), __end));
        (1, 25)
    }
    fn __reduce64<
    >(
        __lookahead_start: Option<&usize>,
        __symbols: &mut alloc::vec::Vec<(usize,__Symbol<>,usize)>,
        _: core::marker::PhantomData<()>,
    ) -> (usize, usize)
    {
        // SubgraphHeader? =  => ActionFn(48);
        let __start = __lookahead_start.cloned().or_else(|| __symbols.last().map(|s| s.2)).unwrap_or_default();
        let __end = __start;
        let __nt = super::__action48::<>(&__start, &__end);
        __symbols.push((__start, __Symbol::Variant28(__nt), __end));
        (0, 25)
    }
}
#[allow(unused_imports)]
pub use self::__parse__FlowchartAst::FlowchartAstParser;

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action0<
>(
    (_, __0, _): (usize, FlowchartAst, usize),
) -> FlowchartAst
{
    __0
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action1<
>(
    (_, _lead, _): (usize, (), usize),
    (_, h, _): (usize, (String, Option<String>, SourceSpan), usize),
    (_, _s, _): (usize, (), usize),
    (_, st, _): (usize, Vec<Stmt>, usize),
) -> FlowchartAst
{
    {
    let (keyword, direction, header_span) = h;
    FlowchartAst { keyword, direction, header_span, statements: st }
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action2<
>(
    (_, l, _): (usize, usize, usize),
    (_, _, _): (usize, Tok, usize),
    (_, d, _): (usize, Option<String>, usize),
    (_, r, _): (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    ("graph".to_string(), d, SourceSpan::new(l, r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action3<
>(
    (_, l, _): (usize, usize, usize),
    (_, _, _): (usize, Tok, usize),
    (_, d, _): (usize, Option<String>, usize),
    (_, r, _): (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    ("flowchart".to_string(), d, SourceSpan::new(l, r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action4<
>(
    (_, l, _): (usize, usize, usize),
    (_, _, _): (usize, Tok, usize),
    (_, d, _): (usize, Option<String>, usize),
    (_, r, _): (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    ("flowchart-elk".to_string(), d, SourceSpan::new(l, r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action5<
>(
    (_, l, _): (usize, usize, usize),
    (_, _, _): (usize, Tok, usize),
    (_, d, _): (usize, Option<String>, usize),
    (_, r, _): (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    ("swimlane-beta".to_string(), d, SourceSpan::new(l, r))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action6<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<Stmt>
{
    Vec::new()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action7<
>(
    (_, s, _): (usize, Stmt, usize),
    (_, rest, _): (usize, Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    {
    let mut st = vec![s];
    st.extend(rest);
    st
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action8<
>(
    (_, _sep, _): (usize, (), usize),
    (_, s, _): (usize, Stmt, usize),
    (_, rest, _): (usize, Vec<Stmt>, usize),
) -> Vec<Stmt>
{
    {
    let mut st = vec![s];
    st.extend(rest);
    st
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action9<
>(
    (_, __0, _): (usize, (), usize),
) -> Vec<Stmt>
{
    Vec::new()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action10<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
)
{
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action11<
>(
    (_, __0, _): (usize, Tok, usize),
    (_, __1, _): (usize, (), usize),
)
{
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action12<
>(
    (_, c, _): (usize, (Vec<Node>, Vec<Edge>), usize),
) -> Stmt
{
    Stmt::Chain { nodes: c.0, edges: c.1 }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action13<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, sd, _): (usize, String, usize),
) -> Stmt
{
    Stmt::ShapeData {
    target: id,
    target_span: Some(SourceSpan::new(l, r)),
    yaml: sd,
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action14<
>(
    (_, g, _): (usize, Vec<Node>, usize),
) -> Stmt
{
    Stmt::Chain { nodes: g, edges: Vec::new() }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action15<
>(
    (_, n, _): (usize, Node, usize),
) -> Stmt
{
    Stmt::Node(Box::new(n))
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action16<
>(
    (_, sg, _): (usize, SubgraphBlock, usize),
) -> Stmt
{
    Stmt::Subgraph(sg)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action17<
>(
    (_, d, _): (usize, DirectionStatementToken, usize),
) -> Stmt
{
    Stmt::Direction(d.direction)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action18<
>(
    (_, s, _): (usize, StyleStmt, usize),
) -> Stmt
{
    Stmt::Style(s)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action19<
>(
    (_, c, _): (usize, ClassDefStmt, usize),
) -> Stmt
{
    Stmt::ClassDef(c)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action20<
>(
    (_, c, _): (usize, ClassAssignStmt, usize),
) -> Stmt
{
    Stmt::ClassAssign(c)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action21<
>(
    (_, c, _): (usize, ClickStmt, usize),
) -> Stmt
{
    Stmt::Click(c)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action22<
>(
    (_, ls, _): (usize, LinkStyleStmt, usize),
) -> Stmt
{
    Stmt::LinkStyle(ls)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action23<
>(
    (_, start, _): (usize, Vec<Node>, usize),
    (_, segs, _): (usize, alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>, usize),
) -> (Vec<Node>, Vec<Edge>)
{
    {
    let mut nodes: Vec<Node> = start.clone();
    let mut edges: Vec<Edge> = Vec::new();

    let mut prev_group = start;
    for (eid, link, label, next_group) in segs {
      for from in &prev_group {
        for to in &next_group {
          let is_last_start = from.id == prev_group[prev_group.len() - 1].id;
          let is_first_end = to.id == next_group[0].id;
          let edge_id = if is_last_start && is_first_end {
            eid.clone()
          } else {
            None
          };
          let edge_label = label.as_ref().map(|l| l.text.clone());
          let label_type = label.as_ref().map(|l| l.kind.clone()).unwrap_or(TitleKind::Text);
          let label_span = label.as_ref().and_then(|l| l.span);
          let label_selection = label.as_ref().and_then(|l| l.selection);
          edges.push(Edge {
            from: from.id.clone(),
            to: to.id.clone(),
            id: edge_id,
            link: link.clone(),
            label: edge_label,
            label_type,
            label_span,
            label_selection,
            style: Vec::new(),
            classes: Vec::new(),
            interpolate: None,
            is_user_defined_id: false,
            animate: None,
            animation: None,
          });
        }
      }
      nodes.extend(next_group.iter().cloned());
      prev_group = next_group;
    }

    (nodes, edges)
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action24<
>(
    (_, __0, _): (usize, Tok, usize),
    (_, __1, _): (usize, (), usize),
)
{
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action25<
>(
    (_, _, _): (usize, Tok, usize),
    (_, h, _): (usize, Option<SubgraphHeader>, usize),
    (_, _s, _): (usize, (), usize),
    (_, inner, _): (usize, Vec<Stmt>, usize),
    (_, _, _): (usize, Tok, usize),
) -> SubgraphBlock
{
    SubgraphBlock { header: h.unwrap_or_default(), statements: inner }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action26<
>(
    (_, eid, _): (usize, Option<String>, usize),
    (_, a, _): (usize, ArrowToken, usize),
    (_, l, _): (usize, Option<LabeledText>, usize),
    (_, n, _): (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    (eid, a.link, l, n)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action27<
>(
    (_, first, _): (usize, Node, usize),
    (_, rest, _): (usize, alloc::vec::Vec<Node>, usize),
) -> Vec<Node>
{
    {
    let mut v = vec![first];
    v.extend(rest);
    v
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action28<
>(
    (_, first, _): (usize, Node, usize),
    (_, rest, _): (usize, alloc::vec::Vec<Node>, usize),
) -> Vec<Node>
{
    {
    let mut v = vec![first];
    v.extend(rest);
    v
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action29<
>(
    (_, _, _): (usize, Tok, usize),
    (_, n, _): (usize, Node, usize),
) -> Node
{
    n
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action30<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, nl, _): (usize, NodeLabelToken, usize),
    (_, sd, _): (usize, Option<String>, usize),
    (_, cls, _): (usize, Vec<String>, usize),
) -> Node
{
    {
    let NodeLabelToken { shape, text, .. } = nl;
    let label_span = text.span;
    let label_selection = text.selection;
    Node {
      id_span: Some(SourceSpan::new(l, r)),
      id,
      provenance: FlowNodeProvenance::Authored,
      syntax: FlowNodeSyntax::ExplicitDefinition,
      label: Some(text.text),
      label_type: text.kind,
      label_span,
      label_selection,
      shape: Some(shape),
      shape_data: sd,
      icon: None,
      form: None,
      pos: None,
      img: None,
      constraint: None,
      asset_width: None,
      asset_height: None,
      styles: Vec::new(),
      classes: cls,
      link: None,
      link_target: None,
      have_callback: false,
    }
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action31<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, cls, _): (usize, Vec<String>, usize),
) -> Node
{
    Node {
    id_span: Some(SourceSpan::new(l, r)),
    id,
    provenance: FlowNodeProvenance::Authored,
    syntax: if cls.is_empty() {
      FlowNodeSyntax::BareReference
    } else {
      FlowNodeSyntax::ExplicitDefinition
    },
    label: None,
    label_type: TitleKind::Text,
    label_span: None,
    label_selection: None,
    shape: None,
    shape_data: None,
    icon: None,
    form: None,
    pos: None,
    img: None,
    constraint: None,
    asset_width: None,
    asset_height: None,
    styles: Vec::new(),
    classes: cls,
    link: None,
    link_target: None,
    have_callback: false,
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action32<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, nl, _): (usize, NodeLabelToken, usize),
    (_, sd, _): (usize, Option<String>, usize),
    (_, cls, _): (usize, Vec<String>, usize),
) -> Node
{
    {
    let NodeLabelToken { shape, text, .. } = nl;
    let label_span = text.span;
    let label_selection = text.selection;
    Node {
      id_span: Some(SourceSpan::new(l, r)),
      id,
      provenance: FlowNodeProvenance::Authored,
      syntax: FlowNodeSyntax::ExplicitDefinition,
      label: Some(text.text),
      label_type: text.kind,
      label_span,
      label_selection,
      shape: Some(shape),
      shape_data: sd,
      icon: None,
      form: None,
      pos: None,
      img: None,
      constraint: None,
      asset_width: None,
      asset_height: None,
      styles: Vec::new(),
      classes: cls,
      link: None,
      link_target: None,
      have_callback: false,
    }
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action33<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, sd, _): (usize, String, usize),
    (_, cls, _): (usize, Vec<String>, usize),
) -> Node
{
    Node {
    id_span: Some(SourceSpan::new(l, r)),
    id,
    provenance: FlowNodeProvenance::Authored,
    syntax: FlowNodeSyntax::ExplicitDefinition,
    label: None,
    label_type: TitleKind::Text,
    label_span: None,
    label_selection: None,
    shape: None,
    shape_data: Some(sd),
    icon: None,
    form: None,
    pos: None,
    img: None,
    constraint: None,
    asset_width: None,
    asset_height: None,
    styles: Vec::new(),
    classes: cls,
    link: None,
    link_target: None,
    have_callback: false,
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action34<
>(
    (_, l, _): (usize, usize, usize),
    (_, id, _): (usize, String, usize),
    (_, r, _): (usize, usize, usize),
    (_, cls, _): (usize, Vec<String>, usize),
) -> Node
{
    Node {
    id_span: Some(SourceSpan::new(l, r)),
    id,
    provenance: FlowNodeProvenance::Authored,
    syntax: if cls.is_empty() {
      FlowNodeSyntax::BareReference
    } else {
      FlowNodeSyntax::ExplicitDefinition
    },
    label: None,
    label_type: TitleKind::Text,
    label_span: None,
    label_selection: None,
    shape: None,
    shape_data: None,
    icon: None,
    form: None,
    pos: None,
    img: None,
    constraint: None,
    asset_width: None,
    asset_height: None,
    styles: Vec::new(),
    classes: cls,
    link: None,
    link_target: None,
    have_callback: false,
  }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action35<
>(
    (_, _, _): (usize, Tok, usize),
    (_, c, _): (usize, String, usize),
) -> Vec<String>
{
    vec![c]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action36<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Vec<String>
{
    Vec::new()
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action37<
>(
    (_, __0, _): (usize, String, usize),
) -> Option<String>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action38<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Option<String>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action39<
>(
    (_, __0, _): (usize, Node, usize),
) -> alloc::vec::Vec<Node>
{
    alloc::vec![__0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action40<
>(
    (_, v, _): (usize, alloc::vec::Vec<Node>, usize),
    (_, e, _): (usize, Node, usize),
) -> alloc::vec::Vec<Node>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action41<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> alloc::vec::Vec<Node>
{
    alloc::vec![]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action42<
>(
    (_, v, _): (usize, alloc::vec::Vec<Node>, usize),
) -> alloc::vec::Vec<Node>
{
    v
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action43<
>(
    (_, __0, _): (usize, LabeledText, usize),
) -> Option<LabeledText>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action44<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Option<LabeledText>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action45<
>(
    (_, __0, _): (usize, String, usize),
) -> Option<String>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action46<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Option<String>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action47<
>(
    (_, __0, _): (usize, SubgraphHeader, usize),
) -> Option<SubgraphHeader>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action48<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Option<SubgraphHeader>
{
    None
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action49<
>(
    (_, __0, _): (usize, (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>), usize),
) -> alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>
{
    alloc::vec![__0]
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action50<
>(
    (_, v, _): (usize, alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>, usize),
    (_, e, _): (usize, (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>), usize),
) -> alloc::vec::Vec<(Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)>
{
    { let mut v = v; v.push(e); v }
}

#[allow(clippy::needless_lifetimes)]
fn __action51<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> usize
{
    *__lookbehind
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action52<
>(
    (_, __0, _): (usize, String, usize),
) -> Option<String>
{
    Some(__0)
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes, clippy::just_underscores_and_digits)]
fn __action53<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> Option<String>
{
    None
}

#[allow(clippy::needless_lifetimes)]
fn __action54<
>(
    __lookbehind: &usize,
    __lookahead: &usize,
) -> usize
{
    *__lookahead
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action55<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
    __2: (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action2(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action56<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
    __2: (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action3(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action57<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
    __2: (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action4(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action58<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
    __2: (usize, usize, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action5(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action59<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, NodeLabelToken, usize),
    __3: (usize, Option<String>, usize),
    __4: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action30(
        __temp0,
        __0,
        __1,
        __2,
        __3,
        __4,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action60<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action31(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action61<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, NodeLabelToken, usize),
    __3: (usize, Option<String>, usize),
    __4: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action32(
        __temp0,
        __0,
        __1,
        __2,
        __3,
        __4,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action62<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, String, usize),
    __3: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action33(
        __temp0,
        __0,
        __1,
        __2,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action63<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action34(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action64<
>(
    __0: (usize, String, usize),
    __1: (usize, usize, usize),
    __2: (usize, String, usize),
) -> Stmt
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action54(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action13(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action65<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.2;
    let __end0 = __1.2;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action55(
        __0,
        __1,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action66<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.2;
    let __end0 = __1.2;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action56(
        __0,
        __1,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action67<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.2;
    let __end0 = __1.2;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action57(
        __0,
        __1,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action68<
>(
    __0: (usize, Tok, usize),
    __1: (usize, Option<String>, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.2;
    let __end0 = __1.2;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action58(
        __0,
        __1,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action69<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, Option<String>, usize),
    __3: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action59(
        __0,
        __temp0,
        __1,
        __2,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action70<
>(
    __0: (usize, String, usize),
    __1: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action60(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action71<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, Option<String>, usize),
    __3: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action61(
        __0,
        __temp0,
        __1,
        __2,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action72<
>(
    __0: (usize, String, usize),
    __1: (usize, String, usize),
    __2: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action62(
        __0,
        __temp0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action73<
>(
    __0: (usize, String, usize),
    __1: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action63(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action74<
>(
    __0: (usize, String, usize),
    __1: (usize, String, usize),
) -> Stmt
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action51(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action64(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action75<
>(
    __0: (usize, Tok, usize),
    __1: (usize, String, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action52(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action65(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action76<
>(
    __0: (usize, Tok, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.2;
    let __end0 = __0.2;
    let __temp0 = __action53(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action65(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action77<
>(
    __0: (usize, Tok, usize),
    __1: (usize, String, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action52(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action66(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action78<
>(
    __0: (usize, Tok, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.2;
    let __end0 = __0.2;
    let __temp0 = __action53(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action66(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action79<
>(
    __0: (usize, Tok, usize),
    __1: (usize, String, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action52(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action67(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action80<
>(
    __0: (usize, Tok, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.2;
    let __end0 = __0.2;
    let __temp0 = __action53(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action67(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action81<
>(
    __0: (usize, Tok, usize),
    __1: (usize, String, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action52(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action68(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action82<
>(
    __0: (usize, Tok, usize),
) -> (String, Option<String>, SourceSpan)
{
    let __start0 = __0.2;
    let __end0 = __0.2;
    let __temp0 = __action53(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action68(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action83<
>(
    __0: (usize, String, usize),
    __1: (usize, ArrowToken, usize),
    __2: (usize, Option<LabeledText>, usize),
    __3: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __0.0;
    let __end0 = __0.2;
    let __temp0 = __action45(
        __0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action26(
        __temp0,
        __1,
        __2,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action84<
>(
    __0: (usize, ArrowToken, usize),
    __1: (usize, Option<LabeledText>, usize),
    __2: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __0.0;
    let __end0 = __0.0;
    let __temp0 = __action46(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action26(
        __temp0,
        __0,
        __1,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action85<
>(
    __0: (usize, String, usize),
    __1: (usize, ArrowToken, usize),
    __2: (usize, LabeledText, usize),
    __3: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __2.0;
    let __end0 = __2.2;
    let __temp0 = __action43(
        __2,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action83(
        __0,
        __1,
        __temp0,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action86<
>(
    __0: (usize, String, usize),
    __1: (usize, ArrowToken, usize),
    __2: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __1.2;
    let __end0 = __2.0;
    let __temp0 = __action44(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action83(
        __0,
        __1,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action87<
>(
    __0: (usize, ArrowToken, usize),
    __1: (usize, LabeledText, usize),
    __2: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action43(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action84(
        __0,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action88<
>(
    __0: (usize, ArrowToken, usize),
    __1: (usize, Vec<Node>, usize),
) -> (Option<String>, LinkToken, Option<LabeledText>, Vec<Node>)
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action44(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action84(
        __0,
        __temp0,
        __1,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action89<
>(
    __0: (usize, Node, usize),
) -> Vec<Node>
{
    let __start0 = __0.2;
    let __end0 = __0.2;
    let __temp0 = __action41(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action27(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action90<
>(
    __0: (usize, Node, usize),
    __1: (usize, alloc::vec::Vec<Node>, usize),
) -> Vec<Node>
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action42(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action27(
        __0,
        __temp0,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action91<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, String, usize),
    __3: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __2.0;
    let __end0 = __2.2;
    let __temp0 = __action37(
        __2,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action69(
        __0,
        __1,
        __temp0,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action92<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __1.2;
    let __end0 = __2.0;
    let __temp0 = __action38(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action69(
        __0,
        __1,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action93<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, String, usize),
    __3: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __2.0;
    let __end0 = __2.2;
    let __temp0 = __action37(
        __2,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action71(
        __0,
        __1,
        __temp0,
        __3,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action94<
>(
    __0: (usize, String, usize),
    __1: (usize, NodeLabelToken, usize),
    __2: (usize, Vec<String>, usize),
) -> Node
{
    let __start0 = __1.2;
    let __end0 = __2.0;
    let __temp0 = __action38(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action71(
        __0,
        __1,
        __temp0,
        __2,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action95<
>(
    __0: (usize, Tok, usize),
    __1: (usize, SubgraphHeader, usize),
    __2: (usize, (), usize),
    __3: (usize, Vec<Stmt>, usize),
    __4: (usize, Tok, usize),
) -> SubgraphBlock
{
    let __start0 = __1.0;
    let __end0 = __1.2;
    let __temp0 = __action47(
        __1,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action25(
        __0,
        __temp0,
        __2,
        __3,
        __4,
    )
}

#[allow(clippy::too_many_arguments, clippy::needless_lifetimes,
    clippy::just_underscores_and_digits)]
fn __action96<
>(
    __0: (usize, Tok, usize),
    __1: (usize, (), usize),
    __2: (usize, Vec<Stmt>, usize),
    __3: (usize, Tok, usize),
) -> SubgraphBlock
{
    let __start0 = __0.2;
    let __end0 = __1.0;
    let __temp0 = __action48(
        &__start0,
        &__end0,
    );
    let __temp0 = (__start0, __temp0, __end0);
    __action25(
        __0,
        __temp0,
        __1,
        __2,
        __3,
    )
}

#[allow(clippy::type_complexity, dead_code)]
pub trait __ToTriple<>
{
    fn to_triple(self) -> Result<(usize,Tok,usize), __lalrpop_util::ParseError<usize, Tok, crate::diagrams::flowchart::LexError>>;
}

impl<> __ToTriple<> for (usize, Tok, usize)
{
    fn to_triple(self) -> Result<(usize,Tok,usize), __lalrpop_util::ParseError<usize, Tok, crate::diagrams::flowchart::LexError>> {
        Ok(self)
    }
}
impl<> __ToTriple<> for Result<(usize, Tok, usize), crate::diagrams::flowchart::LexError>
{
    fn to_triple(self) -> Result<(usize,Tok,usize), __lalrpop_util::ParseError<usize, Tok, crate::diagrams::flowchart::LexError>> {
        self.map_err(|error| __lalrpop_util::ParseError::User { error })
    }
}
